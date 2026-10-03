//! Live H.264 stream of the headset view, shared by all viewers.
//!
//! SteamVR's `steamvr-v4l2cam` renders `IVRHeadsetView` into the v4l2loopback device
//! `/dev/video99` (1920×1080 RGB3, ~90 fps while the headset is active), but only
//! while someone has the device open. The first viewer starts one capture+encode
//! thread; it stops when the last viewer leaves, which also lets v4l2cam go idle.
//!
//! Frames are captured via mmap (no read() copy), paced down to the configured fps,
//! encoded in hardware (see encoder.rs) and broadcast as Annex B access units.
//! Flatpak: needs `--device=all` for /dev/video*.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::encoder::{self, Encoder, RgbFrame};
use crate::hub::Hub;
use crate::v4l2::{self, Mapping, TYPE_CAPTURE, fourcc};

const STATS_INTERVAL: Duration = Duration::from_secs(2);
/// Up to ~2 s of frames per viewer before it lags and resyncs at the next keyframe.
const CHANNEL_CAPACITY: usize = 64;

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub source_device: PathBuf,
    pub encoder_device: String,
    pub fps: u32,
    pub bitrate: u32,
}

/// One encoded frame.
pub struct Packet {
    /// Annex B access unit; keyframes carry SPS/PPS.
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamStats {
    pub viewers: usize,
    pub width: u32,
    pub height: u32,
    pub source_fps: f64,
    pub fps: f64,
    pub kbit_per_sec: f64,
    /// CPU time per frame for RGB→RGBA + queueing.
    pub encode_ms: f64,
}

pub struct LiveStream {
    hub: Arc<Hub>,
    config: StreamConfig,
    running: Mutex<Option<Running>>,
}

struct Running {
    tx: broadcast::Sender<Arc<Packet>>,
    want_keyframe: Arc<AtomicBool>,
}

impl LiveStream {
    pub fn new(hub: Arc<Hub>, config: StreamConfig) -> Arc<Self> {
        Arc::new(Self { hub, config, running: Mutex::new(None) })
    }

    pub fn fps(&self) -> u32 {
        self.config.fps
    }

    /// Joins the stream, starting the pipeline if needed. The next packet sent is a keyframe.
    pub fn subscribe(self: &Arc<Self>) -> broadcast::Receiver<Arc<Packet>> {
        let mut running = self.running.lock().unwrap();
        let running = running.get_or_insert_with(|| self.start());
        running.want_keyframe.store(true, Ordering::Relaxed);
        running.tx.subscribe()
    }

    /// Asks for a keyframe, e.g. after a viewer fell behind.
    pub fn request_keyframe(&self) {
        if let Some(running) = &*self.running.lock().unwrap() {
            running.want_keyframe.store(true, Ordering::Relaxed);
        }
    }

    fn start(self: &Arc<Self>) -> Running {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let want_keyframe = Arc::new(AtomicBool::new(true));
        let (this, thread_tx, thread_want) = (self.clone(), tx.clone(), want_keyframe.clone());
        std::thread::Builder::new()
            .name("stream".into())
            .spawn(move || {
                // Unregisters the pipeline however the thread ends, including a panic, so
                // the next viewer starts a fresh one instead of joining a dead channel.
                let _guard = StopGuard { stream: &this, tx: &thread_tx };
                tracing::info!("stream: starting");
                if let Err(e) = this.run(&thread_tx, &thread_want) {
                    tracing::warn!("stream: {e:#}");
                }
            })
            .expect("spawn stream thread");
        Running { tx, want_keyframe }
    }

    /// Dropping our sender (and the registered clone) ends all viewers.
    fn stop_if_current(&self, tx: &broadcast::Sender<Arc<Packet>>) {
        // Runs during unwinding too: a poisoned lock must not turn into a double panic.
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if running.as_ref().is_some_and(|r| r.tx.same_channel(tx)) {
            *running = None;
        }
        self.hub.update(|s| s.stream = None);
    }

    /// True once the last viewer is gone. Checked under the lock so a concurrent
    /// subscribe() either sees this pipeline still registered or starts a new one.
    fn abandoned(&self, tx: &broadcast::Sender<Arc<Packet>>) -> bool {
        let mut running = self.running.lock().unwrap();
        if tx.receiver_count() > 0 {
            return false;
        }
        if running.as_ref().is_some_and(|r| r.tx.same_channel(tx)) {
            *running = None;
        }
        true
    }

    fn run(&self, tx: &broadcast::Sender<Arc<Packet>>, want_keyframe: &AtomicBool) -> Result<()> {
        let source = LoopbackCapture::open(&self.config.source_device)?;
        let (width, height) = (source.width, source.height);
        let frame_len = source.stride as usize * (height as usize).saturating_sub(1) + width as usize * 3;
        let mut encoder = Encoder::open(&self.config.encoder_device, encoder::Config {
            width,
            height,
            fps: self.config.fps,
            bitrate: self.config.bitrate,
            gop: self.config.fps * 2,
        })?;

        let interval = Duration::from_secs_f64(1.0 / self.config.fps as f64);
        let mut next_due = Instant::now();
        let mut encoded = 0u64;
        let mut window = Instant::now();
        let (mut source_frames, mut frames, mut bytes, mut busy) = (0u32, 0u32, 0usize, Duration::ZERO);

        while !self.abandoned(tx) {
            // Times out while the headset sleeps (no frames); loop to re-check viewers.
            let Some((index, len)) = source.next_frame(Duration::from_secs(2))? else {
                continue;
            };
            source_frames += 1;
            if len < frame_len {
                tracing::debug!("stream: short frame ({len} of {frame_len} bytes), skipped");
                source.release(index)?;
                continue;
            }
            let now = Instant::now();
            if now >= next_due {
                next_due = (next_due + interval).max(now);
                if want_keyframe.swap(false, Ordering::Relaxed) {
                    encoder.force_keyframe();
                }
                let frame = RgbFrame {
                    data: &source.buffers[index as usize].as_slice()[..len],
                    width: width as usize,
                    height: height as usize,
                    stride: source.stride as usize,
                };
                // Nominal timestamps at the configured rate, matching S_PARM.
                let timestamp_us = encoded * 1_000_000 / u64::from(self.config.fps);
                encoded += 1;
                encoder.encode(&frame, timestamp_us, &mut |data, keyframe| {
                    frames += 1;
                    bytes += data.len();
                    let packet = Packet { data: data.to_vec(), keyframe, width, height };
                    let _ = tx.send(Arc::new(packet)); // no receivers is fine
                })?;
                busy += now.elapsed();
            }
            source.release(index)?;

            if window.elapsed() >= STATS_INTERVAL {
                let secs = window.elapsed().as_secs_f64();
                let stats = StreamStats {
                    viewers: tx.receiver_count(),
                    width,
                    height,
                    source_fps: source_frames as f64 / secs,
                    fps: frames as f64 / secs,
                    kbit_per_sec: bytes as f64 * 8.0 / 1000.0 / secs,
                    encode_ms: busy.as_secs_f64() * 1000.0 / frames.max(1) as f64,
                };
                self.hub.update(|s| s.stream = Some(stats));
                (source_frames, frames, bytes, busy) = (0, 0, 0, Duration::ZERO);
                window = Instant::now();
            }
        }
        Ok(())
    }
}

struct StopGuard<'a> {
    stream: &'a LiveStream,
    tx: &'a broadcast::Sender<Arc<Packet>>,
}

impl Drop for StopGuard<'_> {
    fn drop(&mut self) {
        self.stream.stop_if_current(self.tx);
        tracing::info!("stream: stopped");
    }
}

/// mmap streaming capture from the v4l2loopback device.
struct LoopbackCapture {
    buffers: Vec<Mapping>,
    file: File,
    width: u32,
    height: u32,
    stride: u32,
}

impl LoopbackCapture {
    fn open(path: &PathBuf) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true) // needed for PROT_WRITE mappings
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        let fmt = v4l2::get_capture_format(&file)?;
        ensure!(fmt.pixelformat == fourcc(b"RGB3"), "unexpected source format {:08x}", fmt.pixelformat);
        let fd = file.as_raw_fd();
        let buffers = v4l2::request_buffers(fd, TYPE_CAPTURE, 4)?;
        for index in 0..buffers.len() as u32 {
            v4l2::queue(fd, TYPE_CAPTURE, index, 0, 0)?;
        }
        v4l2::stream(fd, TYPE_CAPTURE, true).context("STREAMON source")?;
        tracing::info!("stream: source {}x{} RGB3, {} buffers", fmt.width, fmt.height, buffers.len());
        Ok(Self { buffers, file, width: fmt.width, height: fmt.height, stride: fmt.bytesperline })
    }

    /// Next filled buffer as (index, bytes used); must be handed back with `release`.
    fn next_frame(&self, timeout: Duration) -> Result<Option<(u32, usize)>> {
        let fd = self.file.as_raw_fd();
        if !v4l2::poll(fd, libc::POLLIN, timeout.as_millis() as i32)? {
            return Ok(None);
        }
        Ok(v4l2::dequeue(fd, TYPE_CAPTURE)?.map(|b| (b.index, b.bytesused as usize)))
    }

    fn release(&self, index: u32) -> Result<()> {
        v4l2::queue(self.file.as_raw_fd(), TYPE_CAPTURE, index, 0, 0)
    }
}

impl Drop for LoopbackCapture {
    fn drop(&mut self) {
        let _ = v4l2::stream(self.file.as_raw_fd(), TYPE_CAPTURE, false);
    }
}
