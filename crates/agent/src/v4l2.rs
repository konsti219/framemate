//! Minimal V4L2 ioctl plumbing (aarch64/64-bit layouts), shared by the hardware encoder and the loopback capture

use std::fs::File;
use std::os::fd::{AsRawFd, RawFd};

use anyhow::{Context, Result, bail};

const fn ioc(dir: u64, nr: u64, size: usize) -> u64 {
    dir << 30 | (size as u64) << 16 | (b'V' as u64) << 8 | nr
}
const RW: u64 = 3;
const W: u64 = 1;
pub const VIDIOC_G_FMT: u64 = ioc(RW, 4, size_of::<Format>());
pub const VIDIOC_S_FMT: u64 = ioc(RW, 5, size_of::<Format>());
pub const VIDIOC_REQBUFS: u64 = ioc(RW, 8, size_of::<RequestBuffers>());
pub const VIDIOC_QUERYBUF: u64 = ioc(RW, 9, size_of::<Buffer>());
pub const VIDIOC_QBUF: u64 = ioc(RW, 15, size_of::<Buffer>());
pub const VIDIOC_DQBUF: u64 = ioc(RW, 17, size_of::<Buffer>());
pub const VIDIOC_STREAMON: u64 = ioc(W, 18, size_of::<u32>());
pub const VIDIOC_STREAMOFF: u64 = ioc(W, 19, size_of::<u32>());
pub const VIDIOC_S_PARM: u64 = ioc(RW, 22, size_of::<StreamParm>());
pub const VIDIOC_S_CTRL: u64 = ioc(RW, 28, size_of::<Control>());

pub const TYPE_CAPTURE: u32 = 1;
pub const TYPE_CAPTURE_MPLANE: u32 = 9;
pub const TYPE_OUTPUT_MPLANE: u32 = 10;
pub const MEMORY_MMAP: u32 = 1;
pub const FIELD_NONE: u32 = 1;
pub const BUF_FLAG_KEYFRAME: u32 = 0x8;
pub const BUF_FLAG_LAST: u32 = 0x0010_0000;

fn is_mplane(kind: u32) -> bool {
    kind == TYPE_CAPTURE_MPLANE || kind == TYPE_OUTPUT_MPLANE
}

#[repr(C, align(8))]
pub struct Format {
    pub kind: u32,
    _pad: u32,
    /// union: v4l2_pix_format (single-plane) or packed v4l2_pix_format_mplane
    pub raw: [u8; 200],
}

impl Format {
    pub fn new(kind: u32) -> Self {
        Self { kind, _pad: 0, raw: [0; 200] }
    }
    pub fn mplane(kind: u32, width: u32, height: u32, fourcc: u32) -> Self {
        let mut f = Self::new(kind);
        f.set_u32(0, width);
        f.set_u32(4, height);
        f.set_u32(8, fourcc);
        f.set_u32(12, FIELD_NONE);
        f.raw[180] = 1; // num_planes
        f
    }
    pub fn set_rec709(&mut self) {
        self.set_u32(16, 3); // colorspace REC709
        self.raw[182] = 2; // ycbcr_enc 709
        self.raw[183] = 2; // quantization limited range
        self.raw[184] = 1; // xfer_func 709
    }
    pub fn u32_at(&self, off: usize) -> u32 {
        u32::from_ne_bytes(self.raw[off..off + 4].try_into().unwrap())
    }
    pub fn set_u32(&mut self, off: usize, v: u32) {
        self.raw[off..off + 4].copy_from_slice(&v.to_ne_bytes());
    }
}

#[repr(C)]
pub struct RequestBuffers {
    count: u32,
    kind: u32,
    memory: u32,
    capabilities: u32,
    flags: u8,
    reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Plane {
    bytesused: u32,
    length: u32,
    mem_offset: u64, // union m (mem_offset / userptr / fd)
    data_offset: u32,
    reserved: [u32; 11],
}

#[repr(C)]
pub struct Buffer {
    index: u32,
    kind: u32,
    bytesused: u32,
    flags: u32,
    field: u32,
    _pad: u32,
    timestamp: [i64; 2], // struct timeval
    timecode: [u32; 4],
    sequence: u32,
    memory: u32,
    /// union m: `offset` (single-plane) or `planes` pointer (multiplanar)
    m: u64,
    length: u32,
    reserved2: u32,
    request_fd: i32,
    _pad2: u32,
}

#[repr(C)]
pub struct StreamParm {
    pub kind: u32,
    pub parm: [u32; 50],
}

#[repr(C)]
pub struct Control {
    pub id: u32,
    pub value: i32,
}

const _: () = {
    assert!(size_of::<Format>() == 208);
    assert!(size_of::<RequestBuffers>() == 20);
    assert!(size_of::<Plane>() == 64);
    assert!(size_of::<Buffer>() == 88);
    assert!(size_of::<StreamParm>() == 204);
};

pub fn ioctl<T>(fd: RawFd, request: u64, arg: &mut T) -> std::io::Result<()> {
    loop {
        // SAFETY: `arg` is a correctly sized #[repr(C)] struct for `request`.
        if unsafe { libc::ioctl(fd, request as _, arg as *mut T) } == 0 {
            return Ok(());
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

pub fn set_control(fd: RawFd, id: u32, value: i32) -> std::io::Result<()> {
    ioctl(fd, VIDIOC_S_CTRL, &mut Control { id, value })
}

pub fn stream(fd: RawFd, kind: u32, on: bool) -> std::io::Result<()> {
    ioctl(fd, if on { VIDIOC_STREAMON } else { VIDIOC_STREAMOFF }, &mut kind.clone())
}

pub struct Dequeued {
    pub index: u32,
    pub flags: u32,
    pub bytesused: u32,
    pub data_offset: u32,
}

fn zero_plane() -> Plane {
    Plane { bytesused: 0, length: 0, mem_offset: 0, data_offset: 0, reserved: [0; 11] }
}

fn buffer(kind: u32, index: u32, planes: &mut [Plane; 1]) -> Buffer {
    Buffer {
        index,
        kind,
        bytesused: 0,
        flags: 0,
        field: 0,
        _pad: 0,
        timestamp: [0; 2],
        timecode: [0; 4],
        sequence: 0,
        memory: MEMORY_MMAP,
        m: if is_mplane(kind) { planes.as_mut_ptr() as u64 } else { 0 },
        length: if is_mplane(kind) { 1 } else { 0 },
        reserved2: 0,
        request_fd: 0,
        _pad2: 0,
    }
}

pub fn queue(fd: RawFd, kind: u32, index: u32, bytesused: u32, timestamp_us: u64) -> Result<()> {
    let mut planes = [zero_plane()];
    planes[0].bytesused = bytesused;
    let mut buf = buffer(kind, index, &mut planes);
    buf.bytesused = bytesused;
    buf.timestamp = [(timestamp_us / 1_000_000) as i64, (timestamp_us % 1_000_000) as i64];
    ioctl(fd, VIDIOC_QBUF, &mut buf).with_context(|| format!("QBUF type {kind} index {index}"))
}

/// DQBUF on a non-blocking fd; `None` when nothing is ready.
pub fn dequeue(fd: RawFd, kind: u32) -> Result<Option<Dequeued>> {
    let mut planes = [zero_plane()];
    let mut buf = buffer(kind, 0, &mut planes);
    match ioctl(fd, VIDIOC_DQBUF, &mut buf) {
        Ok(()) => Ok(Some(if is_mplane(kind) {
            Dequeued { index: buf.index, flags: buf.flags, bytesused: planes[0].bytesused, data_offset: planes[0].data_offset }
        } else {
            Dequeued { index: buf.index, flags: buf.flags, bytesused: buf.bytesused, data_offset: 0 }
        })),
        Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => Ok(None),
        // EPIPE: capture queue already returned its LAST buffer.
        Err(e) if e.raw_os_error() == Some(libc::EPIPE) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("DQBUF type {kind}")),
    }
}

pub fn request_buffers(fd: RawFd, kind: u32, count: u32) -> Result<Vec<Mapping>> {
    let mut req = RequestBuffers { count, kind, memory: MEMORY_MMAP, capabilities: 0, flags: 0, reserved: [0; 3] };
    ioctl(fd, VIDIOC_REQBUFS, &mut req).with_context(|| format!("REQBUFS type {kind}"))?;
    (0..req.count)
        .map(|index| {
            let mut planes = [zero_plane()];
            let mut buf = buffer(kind, index, &mut planes);
            ioctl(fd, VIDIOC_QUERYBUF, &mut buf).context("QUERYBUF")?;
            let (offset, len) = if is_mplane(kind) {
                (planes[0].mem_offset as i64, planes[0].length as usize)
            } else {
                (buf.m as u32 as i64, buf.length as usize)
            };
            Mapping::new(fd, offset, len)
        })
        .collect()
}

/// Waits until `fd` is readable/writable or the timeout passes. Returns false on timeout.
pub fn poll(fd: RawFd, events: i16, timeout_ms: i32) -> Result<bool> {
    let mut pfd = libc::pollfd { fd, events, revents: 0 };
    // SAFETY: one valid pollfd.
    let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if rc < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            return Ok(false);
        }
        bail!("poll: {err}");
    }
    if pfd.revents & libc::POLLERR != 0 {
        bail!("device reported POLLERR");
    }
    Ok(rc > 0)
}

pub struct Mapping {
    ptr: *mut u8,
    len: usize,
}

impl Mapping {
    fn new(fd: RawFd, offset: i64, len: usize) -> Result<Self> {
        // SAFETY: mapping a driver buffer at the offset reported by QUERYBUF.
        let ptr = unsafe {
            libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, fd, offset)
        };
        if ptr == libc::MAP_FAILED {
            bail!("mmap: {}", std::io::Error::last_os_error());
        }
        Ok(Self { ptr: ptr.cast(), len })
    }
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: valid mapping for the lifetime of self.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: valid mapping for the lifetime of self, uniquely borrowed.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: unmapping our own mapping.
        unsafe { libc::munmap(self.ptr.cast(), self.len) };
    }
}

// SAFETY: a mapping is plain memory; access is serialized by its owner.
unsafe impl Send for Mapping {}

/// Single-plane capture format (v4l2_pix_format).
pub struct PixFormat {
    pub width: u32,
    pub height: u32,
    pub pixelformat: u32,
    pub bytesperline: u32,
}

pub fn get_capture_format(file: &File) -> Result<PixFormat> {
    let mut f = Format::new(TYPE_CAPTURE);
    ioctl(file.as_raw_fd(), VIDIOC_G_FMT, &mut f).context("VIDIOC_G_FMT")?;
    // v4l2_pix_format: width, height, pixelformat, field, bytesperline, ...
    Ok(PixFormat {
        width: f.u32_at(0),
        height: f.u32_at(4),
        pixelformat: f.u32_at(8),
        bytesperline: f.u32_at(16),
    })
}

pub const fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*code)
}
