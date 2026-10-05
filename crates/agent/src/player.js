// @ts-nocheck: plain browser JS, served as-is by the agent and imported by the app
// Live headset view, shared by the agent's /stream page and the app. The agent sends a JSON header
// with the codec string, an fMP4 init segment, then one moof+mdat per frame.
//
// WebCodecs decodes every frame as it arrives and draws it straight away, about a frame of latency.
// It needs a secure context (HTTPS, localhost, the app, or Chromium's
// --unsafely-treat-insecure-origin-as-secure), so elsewhere (and in Firefox, which lacks it) the
// stream goes through Media Source Extensions, which buffers and has to be kept near the live edge.

/** Calls `onStatus` with "" once playing, or a message. Returns `{ close() }`. */
export function startPlayer({ url, video, canvas, onStatus }) {
  let ws = null;
  let retry;
  let closed = false;
  let teardown = () => {};

  const connect = () => {
    ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";
    // The header comes first and picks the decoder; picking is async, so frames that arrive meanwhile wait
    ws.onmessage = async e => {
      if (typeof e.data !== "string") return;
      teardown();
      const { codec } = JSON.parse(e.data);
      const pending = [];
      let handler = null;
      ws.onmessage = m => (handler ? handler(m.data) : pending.push(m.data));
      handler = await choose(codec);
      if (handler) pending.splice(0).forEach(handler);
    };
    ws.onclose = () => {
      teardown();
      if (closed) return;
      onStatus("Reconnecting…");
      retry = setTimeout(connect, 2000);
    };
  };

  const choose = async codec => {
    if (typeof VideoDecoder !== "undefined" && (await VideoDecoder.isConfigSupported({ codec })).supported) {
      return webCodecs(codec);
    }
    const type = `video/mp4; codecs="${codec}"`;
    if (typeof MediaSource !== "undefined" && MediaSource.isTypeSupported(type)) return mse(type);
    onStatus(`Can't play ${codec} here`);
    closed = true;
    ws.close();
    return null;
  };

  const webCodecs = codec => {
    video.hidden = true;
    canvas.hidden = false;
    const ctx = canvas.getContext("2d");
    let lengthSize = 4;
    let haveKey = false;
    let n = 0;
    const decoder = new VideoDecoder({
      output: frame => {
        if (canvas.width !== frame.displayWidth || canvas.height !== frame.displayHeight) {
          canvas.width = frame.displayWidth;
          canvas.height = frame.displayHeight;
        }
        ctx.drawImage(frame, 0, 0);
        frame.close();
      },
      error: err => {
        onStatus(`Decoder error: ${err.message}`);
        ws.close();
      },
    });
    teardown = () => {
      if (decoder.state !== "closed") decoder.close();
      teardown = () => {};
    };
    onStatus("");
    return data => {
      const bytes = new Uint8Array(data);
      if (decoder.state === "unconfigured") {
        const avcC = findBox(bytes, "avcC");
        if (!avcC) return;
        lengthSize = (avcC[4] & 3) + 1;
        decoder.configure({ codec, description: avcC, optimizeForLatency: true });
        return;
      }
      const mdat = topLevelBox(bytes, "mdat");
      if (!mdat) return;
      const key = hasIdr(mdat, lengthSize);
      // Fell behind (slow device or a burst): skip ahead to the next keyframe
      if (decoder.decodeQueueSize > 4 && !key) haveKey = false;
      if ((!haveKey && !key) || decoder.state !== "configured") return;
      haveKey = true;
      decoder.decode(new EncodedVideoChunk({ type: key ? "key" : "delta", timestamp: n++ * 33333, data: mdat }));
    };
  };

  const mse = type => {
    canvas.hidden = true;
    video.hidden = false;
    const source = new MediaSource();
    const objectUrl = URL.createObjectURL(source);
    video.src = objectUrl;
    let buffer = null;
    const queue = [];
    const pump = () => {
      if (buffer && !buffer.updating && queue.length) buffer.appendBuffer(queue.shift());
    };
    source.addEventListener(
      "sourceopen",
      () => {
        buffer = source.addSourceBuffer(type);
        buffer.mode = "sequence";
        buffer.addEventListener("updateend", () => {
          keepLive(buffer);
          pump();
        });
        pump();
      },
      { once: true },
    );
    teardown = () => {
      URL.revokeObjectURL(objectUrl);
      teardown = () => {};
    };
    onStatus("");
    return data => {
      queue.push(data);
      pump();
    };
  };

  // MSE buffers. Play faster as soon as we're more than a few frames behind; seek only when far
  // behind, since a seek restarts decoding at the previous keyframe (seeking on every frame gave ~3 fps).
  const keepLive = buffer => {
    if (buffer.updating || !buffer.buffered.length) return;
    const start = buffer.buffered.start(0);
    const end = buffer.buffered.end(buffer.buffered.length - 1);
    const behind = end - video.currentTime;
    if (video.currentTime < start || behind > 1) video.currentTime = Math.max(start, end - 0.05);
    video.playbackRate = behind > 0.25 ? 1.5 : behind > 0.1 ? 1.15 : 1.0;
    if (video.paused) video.play().catch(() => {});
    if (video.currentTime - start > 10) buffer.remove(start, video.currentTime - 5);
  };

  connect();
  return {
    close() {
      closed = true;
      clearTimeout(retry);
      teardown();
      ws?.close();
    },
  };
}

// fMP4 is a tree of [size u32][type][payload] boxes. avcC sits deep in the init segment, but its
// fourcc doesn't occur elsewhere there, so a byte search is enough.
function findBox(bytes, type) {
  const t = [...type].map(c => c.charCodeAt(0));
  for (let i = 4; i + 4 <= bytes.length; i++) {
    if (bytes[i] === t[0] && bytes[i + 1] === t[1] && bytes[i + 2] === t[2] && bytes[i + 3] === t[3]) {
      const size = readU32(bytes, i - 4);
      if (size >= 8 && i - 4 + size <= bytes.length) return bytes.subarray(i + 4, i - 4 + size);
    }
  }
  return null;
}

function topLevelBox(bytes, type) {
  for (let off = 0; off + 8 <= bytes.length; ) {
    const size = readU32(bytes, off);
    if (size < 8) return null;
    if (String.fromCharCode(...bytes.subarray(off + 4, off + 8)) === type) return bytes.subarray(off + 8, off + size);
    off += size;
  }
  return null;
}

// The sample is length-prefixed NAL units; type 5 is an IDR slice
function hasIdr(sample, lengthSize) {
  for (let off = 0; off + lengthSize < sample.length; ) {
    let len = 0;
    for (let i = 0; i < lengthSize; i++) len = len * 256 + sample[off + i];
    if ((sample[off + lengthSize] & 0x1f) === 5) return true;
    off += lengthSize + len;
  }
  return false;
}

function readU32(b, off) {
  return ((b[off] << 24) | (b[off + 1] << 16) | (b[off + 2] << 8) | b[off + 3]) >>> 0;
}
