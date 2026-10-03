//! HTTP API and plain-text dashboard.
//!
//! - `GET /`            dashboard page (token is read from `?token=` by the page itself)
//! - `GET /api/state`   full state as JSON
//! - `GET /api/ws`      full state as JSON on connect and after every change (throttled)
//! - `GET /stream`      headset-view player page (token read from `?token=` by the page)
//! - `GET /api/stream/ws` headset view: JSON `{codec}`, fMP4 init segment, then one
//!   moof+mdat per frame (see stream.rs, fmp4.rs)
//! - `GET /favicon.svg` logo from `assets/`, no auth
//! - `GET /healthz`     liveness, no auth
//!
//! `/api/*` requires the token via `?token=` or `Authorization: Bearer`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use socket2::{Domain, Protocol, Socket, Type};

use crate::config::Config;
use crate::fmp4;
use crate::hub::Hub;
use crate::stream::LiveStream;

/// Coalesces bursts (download progress fires every second) into one push.
const PUSH_THROTTLE: Duration = Duration::from_millis(250);

#[derive(Clone)]
struct AppState {
    hub: Arc<Hub>,
    token: Arc<str>,
    stream: Arc<LiveStream>,
}

pub async fn serve(hub: Arc<Hub>, stream: Arc<LiveStream>, config: &Config) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(|| async { Html(include_str!("dashboard.html")) }))
        .route("/api/state", get(state))
        .route("/api/ws", get(ws))
        .route("/stream", get(|| async { Html(include_str!("stream.html")) }))
        .route("/api/stream/ws", get(stream_ws))
        .route("/favicon.svg", get(|| async { asset("image/svg+xml", include_bytes!("../../../assets/framemate-black.svg")) }))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(AppState {
            hub,
            token: config.token.as_str().into(),
            stream,
        });

    let listener = listen(config.listen)?;
    let token = crate::config::format_token(&config.token);
    tracing::info!("listening on http://{}/?token={token} (token: {token})", config.listen);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Binds the listener, accepting IPv4 *and* IPv6 when given an IPv6 wildcard address.
///
/// `frame.local` resolves to an AAAA record on many networks (and Chrome prefers it), while
/// the app is usually handed an IPv4 address, so the agent has to answer on both families.
/// A dual-stack socket needs `IPV6_V6ONLY` cleared before `bind`, which
/// `TcpListener::bind` can't express; leaving it to the `net.ipv6.bindv6only` sysctl would
/// silently drop IPv4 on a host that has it set.
fn listen(addr: SocketAddr) -> anyhow::Result<tokio::net::TcpListener> {
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))
        .context("creating the listening socket")?;
    if addr.is_ipv6() {
        socket.set_only_v6(false).context("clearing IPV6_V6ONLY")?;
    }
    // A restart must not fail while the previous socket lingers.
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into()).with_context(|| format!("binding {addr}"))?;
    socket.listen(1024)?;
    Ok(tokio::net::TcpListener::from_std(socket.into())?)
}

fn asset(content_type: &'static str, body: &'static [u8]) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "public, max-age=86400")],
        body,
    )
}

fn authorized(app: &AppState, headers: &HeaderMap, query: &HashMap<String, String>) -> bool {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    bearer
        .or(query.get("token").map(String::as_str))
        .is_some_and(|token| crate::config::normalize_token(token) == *app.token)
}

async fn state(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    // CORS: the app's WebView (another origin) reads the status to tell a wrong token from
    // an unreachable agent. Harmless, the token is still required.
    let cors = [(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")];
    if !authorized(&app, &headers, &query) {
        return (StatusCode::UNAUTHORIZED, cors).into_response();
    }
    (cors, [(header::CONTENT_TYPE, "application/json")], app.hub.snapshot_json()).into_response()
}

async fn ws(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !authorized(&app, &headers, &query) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    upgrade.on_upgrade(move |socket| push_state(socket, app.hub))
}

async fn push_state(mut socket: WebSocket, hub: Arc<Hub>) {
    let mut changes = hub.subscribe();
    changes.mark_unchanged();
    loop {
        if socket.send(Message::Text(hub.snapshot_json().into())).await.is_err() {
            return;
        }
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return;
                }
                tokio::time::sleep(PUSH_THROTTLE).await;
                changes.mark_unchanged();
            }
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            },
        }
    }
}

async fn stream_ws(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !authorized(&app, &headers, &query) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    upgrade.on_upgrade(move |socket| push_stream(socket, app.stream))
}

/// Remuxes the shared H.264 stream into fMP4 for one viewer. Starts at a keyframe;
/// if the viewer falls behind it skips ahead to the next keyframe.
async fn push_stream(mut socket: WebSocket, stream: Arc<LiveStream>) {
    use tokio::sync::broadcast::error::RecvError;

    let frame_duration = fmp4::TIMESCALE / stream.fps();
    let mut packets = stream.subscribe();
    let (mut initialized, mut synced) = (false, false);
    let (mut sequence, mut decode_time) = (1u32, 0u64);
    loop {
        let packet = tokio::select! {
            packet = packets.recv() => match packet {
                Ok(packet) => packet,
                Err(RecvError::Lagged(_)) => {
                    synced = false;
                    stream.request_keyframe();
                    continue;
                }
                Err(RecvError::Closed) => return,
            },
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => continue,
            },
        };
        if !synced {
            if !packet.keyframe {
                continue;
            }
            synced = true;
        }
        if !initialized {
            let Some((sps, pps)) = fmp4::parameter_sets(&packet.data) else {
                tracing::warn!("stream: keyframe without SPS/PPS");
                continue;
            };
            let header = serde_json::json!({ "codec": fmp4::codec_string(sps) }).to_string();
            let init = fmp4::init_segment(packet.width, packet.height, sps, pps);
            if socket.send(Message::Text(header.into())).await.is_err()
                || socket.send(Message::Binary(init.into())).await.is_err()
            {
                return;
            }
            initialized = true;
        }
        let segment = fmp4::media_segment(sequence, decode_time, frame_duration, packet.keyframe, &fmp4::to_sample(&packet.data));
        sequence += 1;
        decode_time += u64::from(frame_duration);
        if socket.send(Message::Binary(segment.into())).await.is_err() {
            return;
        }
    }
}

async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
    tracing::info!("shutting down");
}
