//! Minimal fragmented-MP4 writer for one H.264 video track, as consumed by browser
//! Media Source Extensions: one init segment (ftyp+moov), then one moof+mdat per frame.

/// Timescale of the video track (90 kHz, the usual for video).
pub const TIMESCALE: u32 = 90_000;

/// Splits an Annex B access unit into NAL units (without start codes).
pub fn nal_units(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    (0..starts.len()).map(move |n| {
        let end = starts.get(n + 1).map_or(data.len(), |&next| next - 3);
        let mut nal = &data[starts[n]..end];
        // A 4-byte start code leaves a zero byte at the end of the previous NAL.
        while let [rest @ .., 0] = nal {
            nal = rest;
        }
        nal
    })
}

fn nal_type(nal: &[u8]) -> u8 {
    nal.first().map_or(0, |b| b & 0x1f)
}

/// SPS and PPS from an access unit (the encoder prepends them to every keyframe).
pub fn parameter_sets(au: &[u8]) -> Option<(&[u8], &[u8])> {
    let sps = nal_units(au).find(|n| nal_type(n) == 7)?;
    let pps = nal_units(au).find(|n| nal_type(n) == 8)?;
    Some((sps, pps))
}

/// RFC 6381 codec string, e.g. `avc1.640028`.
pub fn codec_string(sps: &[u8]) -> String {
    format!("avc1.{:02x}{:02x}{:02x}", sps[1], sps[2], sps[3])
}

/// Annex B -> length-prefixed sample; parameter sets and AUDs live in the init segment.
pub fn to_sample(au: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(au.len() + 16);
    for nal in nal_units(au).filter(|n| !matches!(nal_type(n), 7..=9)) {
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
    out
}

pub fn init_segment(width: u32, height: u32, sps: &[u8], pps: &[u8]) -> Vec<u8> {
    let ftyp = bx(b"ftyp", &[b"isom".as_slice(), &0x200u32.to_be_bytes(), b"isom", b"iso6", b"avc1", b"mp41"].concat());

    let matrix: Vec<u8> = [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000]
        .iter()
        .flat_map(|v| v.to_be_bytes())
        .collect();
    let mvhd = full(b"mvhd", 0, 0, &[
        &[0u8; 8][..],                   // creation/modification time
        &1000u32.to_be_bytes(),          // timescale
        &0u32.to_be_bytes(),             // duration
        &0x0001_0000u32.to_be_bytes(),   // rate 1.0
        &0x0100u16.to_be_bytes(),        // volume 1.0
        &[0u8; 10],                      // reserved
        &matrix,
        &[0u8; 24],                      // pre_defined
        &2u32.to_be_bytes(),             // next_track_ID
    ].concat());
    let tkhd = full(b"tkhd", 0, 3, &[
        &[0u8; 8][..],                   // creation/modification time
        &1u32.to_be_bytes(),             // track_ID
        &[0u8; 4],                       // reserved
        &0u32.to_be_bytes(),             // duration
        &[0u8; 8],                       // reserved
        &[0u8; 8],                       // layer, alternate_group, volume, reserved
        &matrix,
        &(width << 16).to_be_bytes(),
        &(height << 16).to_be_bytes(),
    ].concat());
    let mdhd = full(b"mdhd", 0, 0, &[
        &[0u8; 8][..],
        &TIMESCALE.to_be_bytes(),
        &0u32.to_be_bytes(),
        &0x55c4u16.to_be_bytes(),        // language "und"
        &[0u8; 2],
    ].concat());
    let hdlr = full(b"hdlr", 0, 0, &[&[0u8; 4][..], b"vide", &[0u8; 12], b"FrameMate\0"].concat());
    let vmhd = full(b"vmhd", 0, 1, &[0u8; 8]);
    let dinf = bx(b"dinf", &full(b"dref", 0, 0, &[&1u32.to_be_bytes()[..], &full(b"url ", 0, 1, &[])].concat()));

    let avcc = bx(b"avcC", &[
        &[1, sps[1], sps[2], sps[3], 0xff, 0xe1][..], // version, profile, compat, level, 4-byte lengths, 1 SPS
        &(sps.len() as u16).to_be_bytes(),
        sps,
        &[1],                                          // 1 PPS
        &(pps.len() as u16).to_be_bytes(),
        pps,
    ].concat());
    let avc1 = bx(b"avc1", &[
        &[0u8; 6][..],                   // reserved
        &1u16.to_be_bytes(),             // data_reference_index
        &[0u8; 16],                      // pre_defined, reserved
        &(width as u16).to_be_bytes(),
        &(height as u16).to_be_bytes(),
        &0x0048_0000u32.to_be_bytes(),   // 72 dpi
        &0x0048_0000u32.to_be_bytes(),
        &[0u8; 4],                       // reserved
        &1u16.to_be_bytes(),             // frame_count
        &[0u8; 32],                      // compressorname
        &0x0018u16.to_be_bytes(),        // depth
        &0xffffu16.to_be_bytes(),        // pre_defined = -1
        &avcc,
    ].concat());
    let stbl = bx(b"stbl", &[
        full(b"stsd", 0, 0, &[&1u32.to_be_bytes()[..], &avc1].concat()),
        full(b"stts", 0, 0, &[0u8; 4]),
        full(b"stsc", 0, 0, &[0u8; 4]),
        full(b"stsz", 0, 0, &[0u8; 8]),
        full(b"stco", 0, 0, &[0u8; 4]),
    ].concat());
    let minf = bx(b"minf", &[vmhd, dinf, stbl].concat());
    let mdia = bx(b"mdia", &[mdhd, hdlr, minf].concat());
    let trak = bx(b"trak", &[tkhd, mdia].concat());
    let trex = full(b"trex", 0, 0, &[1u32, 1, 0, 0, 0].iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<_>>());
    let moov = bx(b"moov", &[mvhd, trak, bx(b"mvex", &trex)].concat());
    [ftyp, moov].concat()
}

/// One frame as moof+mdat.
pub fn media_segment(sequence: u32, decode_time: u64, duration: u32, keyframe: bool, sample: &[u8]) -> Vec<u8> {
    // sample_flags: sync sample vs. "depends on others, non-sync".
    let flags: u32 = if keyframe { 0x0200_0000 } else { 0x0101_0000 };
    let trun_body = |data_offset: u32| {
        [1u32, data_offset, duration, sample.len() as u32, flags]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect::<Vec<_>>()
    };
    let moof = |data_offset: u32| {
        bx(b"moof", &[
            full(b"mfhd", 0, 0, &sequence.to_be_bytes()),
            bx(b"traf", &[
                full(b"tfhd", 0, 0x02_0000, &1u32.to_be_bytes()), // default-base-is-moof, track 1
                full(b"tfdt", 1, 0, &decode_time.to_be_bytes()),
                // data-offset, sample duration, size and flags present
                full(b"trun", 0, 0x0701, &trun_body(data_offset)),
            ].concat()),
        ].concat())
    };
    // The data offset points past moof and the mdat header; moof's size doesn't depend on it.
    let size = moof(0).len() as u32;
    [moof(size + 8), bx(b"mdat", sample)].concat()
}

fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    [&((body.len() + 8) as u32).to_be_bytes()[..], kind, body].concat()
}

fn full(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let header = (u32::from(version) << 24 | flags).to_be_bytes();
    bx(kind, &[&header[..], body].concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_annex_b_with_mixed_start_codes() {
        let au = [0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 1, 0x65, 4, 5];
        let nals: Vec<_> = nal_units(&au).collect();
        assert_eq!(nals, [&[0x67, 1, 2][..], &[0x68, 3], &[0x65, 4, 5]]);
        let (sps, pps) = parameter_sets(&au).unwrap();
        assert_eq!((sps, pps), (&[0x67, 1, 2][..], &[0x68, 3][..]));
        assert_eq!(to_sample(&au), [0, 0, 0, 3, 0x65, 4, 5]);
    }

    #[test]
    fn segment_data_offset_points_at_sample() {
        let seg = media_segment(1, 0, 3000, true, &[0xaa, 0xbb]);
        let moof_len = u32::from_be_bytes(seg[0..4].try_into().unwrap()) as usize;
        // trun's data_offset is the 4 bytes after its sample_count (16 bytes before moof's end).
        let offset = u32::from_be_bytes(seg[moof_len - 16..moof_len - 12].try_into().unwrap()) as usize;
        assert_eq!(&seg[offset..], &[0xaa, 0xbb]);
    }
}
