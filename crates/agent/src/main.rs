mod cdp;
mod config;
mod encoder;
mod fmp4;
mod hub;
mod power;
mod server;
mod steamos;
mod stream;
mod v4l2;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Under systemd, journald adds timestamps and doesn't render ANSI colors.
    let under_systemd = std::env::var_os("INVOCATION_ID").is_some();
    let log = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("FRAMEMATE_LOG").unwrap_or_else(|_| "info".into()))
        .with_ansi(!under_systemd);
    if under_systemd {
        log.without_time().init();
    } else {
        log.init();
    }

    let config = config::Config::from_env()?;
    let hub = hub::Hub::new(config::hostname());
    let stream = stream::LiveStream::new(hub.clone(), config.stream.clone());

    tokio::spawn(cdp::run(hub.clone(), config.cdp_url.clone()));
    tokio::spawn(power::run(hub.clone(), config.power_supply_dir.clone()));
    tokio::spawn(steamos::run(hub.clone()));

    server::serve(hub, stream, &config).await
}
