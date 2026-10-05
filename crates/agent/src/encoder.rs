//! H.264 on the `iris` V4L2 stateful encoder (`/dev/video23`) via raw multiplanar M2M ioctls
//! (GStreamer's `v4l2h264enc` can't negotiate with this driver). Input is RGBA; the encoder
//! does the YUV conversion.
//!
//! Driver quirks: STREAMOFF on a never-started queue returns EBUSY; height is aligned to 16
//! (1080 → 1088) and the SPS crops it back.

use std::fs::{File, OpenOptions};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;

use anyhow::{Context, Result, bail};

use crate::v4l2::{self, Format, Mapping, StreamParm, TYPE_CAPTURE_MPLANE, TYPE_OUTPUT_MPLANE, fourcc};

const CID_B_FRAMES: u32 = 0x0099_09ca;
const CID_GOP_SIZE: u32 = 0x0099_09cb;
const CID_BITRATE_MODE: u32 = 0x0099_09ce; // 0 VBR, 1 CBR
const CID_BITRATE: u32 = 0x0099_09cf;
const CID_FORCE_KEY_FRAME: u32 = 0x0099_09e5;
const CID_H264_PROFILE: u32 = 0x0099_0a6b; // 4 = High
const CID_PREPEND_SPS_PPS_TO_IDR: u32 = 0x0099_0b84;

const BUFFER_COUNT: u32 = 4;
/// How long `encode` waits for the frame it just queued; encoding 1080p takes a few ms.
const OUTPUT_WAIT_MS: u32 = 25;

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub gop: u32,
}

/// A source frame in packed 24-bit RGB.
pub struct RgbFrame<'a> {
    pub data: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
}

pub struct Encoder {
    file: File,
    /// Driver-aligned input layout.
    height: usize,
    stride: usize,
    sizeimage: usize,
    outputs: Vec<Mapping>,
    free_outputs: Vec<u32>,
    captures: Vec<Mapping>,
}

impl Encoder {
    pub fn open(path: &str, config: Config) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .with_context(|| format!("opening {path}"))?;
        let fd = file.as_raw_fd();

        // Coded format first, then the raw input format (stateful encoder sequence).
        let mut capture = Format::mplane(TYPE_CAPTURE_MPLANE, config.width, config.height, fourcc(b"H264"));
        v4l2::ioctl(fd, v4l2::VIDIOC_S_FMT, &mut capture).context("S_FMT capture")?;
        let mut output = Format::mplane(TYPE_OUTPUT_MPLANE, config.width, config.height, fourcc(b"AB24"));
        output.set_rec709();
        v4l2::ioctl(fd, v4l2::VIDIOC_S_FMT, &mut output).context("S_FMT output")?;
        if output.u32_at(8) != fourcc(b"AB24") {
            bail!("encoder rejected RGBA input");
        }

        let mut parm = StreamParm { kind: TYPE_OUTPUT_MPLANE, parm: [0; 50] };
        parm.parm[2] = 1; // timeperframe numerator
        parm.parm[3] = config.fps; // timeperframe denominator
        if let Err(e) = v4l2::ioctl(fd, v4l2::VIDIOC_S_PARM, &mut parm) {
            tracing::warn!("encoder: S_PARM: {e}");
        }
        for (name, id, value) in [
            ("bitrate mode", CID_BITRATE_MODE, 1),
            ("bitrate", CID_BITRATE, config.bitrate as i32),
            ("gop", CID_GOP_SIZE, config.gop as i32),
            ("b-frames", CID_B_FRAMES, 0),
            ("profile", CID_H264_PROFILE, 4),
            // Every keyframe carries SPS/PPS, so viewers can join mid-stream.
            ("prepend sps/pps", CID_PREPEND_SPS_PPS_TO_IDR, 1),
        ] {
            if let Err(e) = v4l2::set_control(fd, id, value) {
                tracing::warn!("encoder: setting {name}: {e}");
            }
        }

        let outputs = v4l2::request_buffers(fd, TYPE_OUTPUT_MPLANE, BUFFER_COUNT)?;
        let captures = v4l2::request_buffers(fd, TYPE_CAPTURE_MPLANE, BUFFER_COUNT)?;
        for index in 0..captures.len() as u32 {
            v4l2::queue(fd, TYPE_CAPTURE_MPLANE, index, 0, 0)?;
        }
        v4l2::stream(fd, TYPE_OUTPUT_MPLANE, true).context("STREAMON output")?;
        v4l2::stream(fd, TYPE_CAPTURE_MPLANE, true).context("STREAMON capture")?;

        Ok(Self {
            height: output.u32_at(4) as usize,
            stride: output.u32_at(24) as usize,
            sizeimage: output.u32_at(20) as usize,
            free_outputs: (0..outputs.len() as u32).rev().collect(),
            outputs,
            captures,
            file,
        })
    }

    pub fn force_keyframe(&self) {
        if let Err(e) = v4l2::set_control(self.fd(), CID_FORCE_KEY_FRAME, 1) {
            tracing::warn!("encoder: force keyframe: {e}");
        }
    }

    /// `sink` gets each finished access unit (Annex B) and whether it's a keyframe.
    /// Timestamps must increase: rate control uses them (constant ones undershoot ~5×).
    pub fn encode(&mut self, frame: &RgbFrame, timestamp_us: u64, sink: &mut impl FnMut(&[u8], bool)) -> Result<()> {
        let index = loop {
            if let Some(index) = self.free_outputs.pop() {
                break index;
            }
            if !v4l2::poll(self.fd(), libc::POLLIN | libc::POLLOUT, 2000)? {
                bail!("encoder stalled");
            }
            self.service(sink)?;
        };
        rgb_to_rgba(frame, self.outputs[index as usize].as_mut_slice(), self.stride, self.height);
        v4l2::queue(self.fd(), TYPE_OUTPUT_MPLANE, index, self.sizeimage as u32, timestamp_us)?;
        // Hand this frame's output on now rather than with the next frame, which costs a whole frame
        // interval. Short sleeps instead of poll(): the m2m poll reports POLLERR once the encoder has
        // taken the input buffer and the output queue is empty.
        for _ in 0..OUTPUT_WAIT_MS {
            if self.service(sink)? > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    }

    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Reclaims consumed input buffers and delivers finished output, without blocking.
    /// Returns how many encoded frames went to `sink`.
    fn service(&mut self, sink: &mut impl FnMut(&[u8], bool)) -> Result<usize> {
        let fd = self.fd();
        let mut delivered = 0;
        while let Some(buf) = v4l2::dequeue(fd, TYPE_OUTPUT_MPLANE)? {
            self.free_outputs.push(buf.index);
        }
        while let Some(buf) = v4l2::dequeue(fd, TYPE_CAPTURE_MPLANE)? {
            let (start, end) = (buf.data_offset as usize, buf.bytesused as usize);
            if end > start {
                let data = &self.captures[buf.index as usize].as_slice()[start..end];
                sink(data, buf.flags & v4l2::BUF_FLAG_KEYFRAME != 0);
                delivered += 1;
            }
            if buf.flags & v4l2::BUF_FLAG_LAST == 0 {
                v4l2::queue(fd, TYPE_CAPTURE_MPLANE, buf.index, 0, 0)?;
            }
        }
        Ok(delivered)
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // Both queues were started in open(), so STREAMOFF is safe here.
        let _ = v4l2::stream(self.fd(), TYPE_CAPTURE_MPLANE, false);
        let _ = v4l2::stream(self.fd(), TYPE_OUTPUT_MPLANE, false);
    }
}

/// RGB3 → RGBA into the encoder's input buffer; rows past the source are black.
fn rgb_to_rgba(frame: &RgbFrame, dst: &mut [u8], stride: usize, height: usize) {
    for y in 0..height {
        let row = &mut dst[y * stride..y * stride + frame.width * 4];
        if y >= frame.height {
            row.fill(0);
            continue;
        }
        let src = &frame.data[y * frame.stride..y * frame.stride + frame.width * 3];
        let mut dst_chunks = row.chunks_exact_mut(16);
        let mut src_chunks = src.chunks_exact(12);
        for (d, s) in (&mut dst_chunks).zip(&mut src_chunks) {
            let px = |i: usize| [s[i], s[i + 1], s[i + 2], 255];
            d[0..4].copy_from_slice(&px(0));
            d[4..8].copy_from_slice(&px(3));
            d[8..12].copy_from_slice(&px(6));
            d[12..16].copy_from_slice(&px(9));
        }
        for (d, s) in dst_chunks.into_remainder().chunks_exact_mut(4).zip(src_chunks.remainder().chunks_exact(3)) {
            d.copy_from_slice(&[s[0], s[1], s[2], 255]);
        }
    }
}
