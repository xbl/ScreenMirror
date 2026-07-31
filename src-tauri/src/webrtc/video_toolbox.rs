//! H.264 helpers and the re-exported `NativeVideoEncoder`.
//!
//! `VideoEncoder` is now a thin alias for the bindgen-driven
//! [`NativeVideoEncoder`](crate::webrtc::video_toolbox_native::NativeVideoEncoder)
//! which wraps a persistent `AVCodecContext` instead of spawning per-frame
//! `ffmpeg` processes. The Annex B / AVCC helpers and their unit tests
//! remain here because `NativeVideoEncoder` (and the rest of the WebRTC
//! pipeline) depends on them.

#[derive(Debug, Clone)]
pub struct H264EncodedFrame {
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub captured_at: std::time::Instant,
}

pub fn split_annex_b_nalus(input: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= input.len() {
        let len = if input[i..].starts_with(&[0, 0, 1]) {
            3
        } else if i + 4 <= input.len() && input[i..].starts_with(&[0, 0, 0, 1]) {
            4
        } else {
            i += 1;
            continue;
        };
        starts.push((i, len));
        i += len;
    }
    starts
        .iter()
        .enumerate()
        .filter_map(|(index, &(start, prefix))| {
            let end = starts
                .get(index + 1)
                .map(|(next, _)| *next)
                .unwrap_or(input.len());
            let nal = &input[start + prefix..end];
            (!nal.is_empty()).then_some(nal)
        })
        .collect()
}

pub fn is_keyframe(sample: &[u8]) -> bool {
    split_annex_b_nalus(sample)
        .iter()
        .any(|nal| matches!(nal.first().map(|v| v & 0x1f), Some(5)))
}

pub fn annex_b_to_avcc(sample: &[u8]) -> Result<Vec<u8>, String> {
    let nalus = split_annex_b_nalus(sample);
    if nalus.is_empty() {
        return Err("H.264 sample contains no NAL units".into());
    }
    let size = nalus.iter().map(|n| 4 + n.len()).sum();
    let mut out = Vec::with_capacity(size);
    for nal in nalus {
        let len = u32::try_from(nal.len()).map_err(|_| "NAL unit is too large")?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(nal);
    }
    Ok(out)
}

/// Normalize H.264 codec extradata to Annex B form, regardless of whether
/// the encoder hands us AVCC (length-prefixed) or Annex B (start-code-
/// prefixed) bytes.
///
/// `h264_videotoolbox` in libavcodec has historically written its extradata
/// as a single Annex B blob, but this is implementation-defined: a future
/// libavcodec, a different encoder backend, or any future H.264 codec that
/// adopts `AV_CODEC_FLAG_GLOBAL_HEADER` will emit AVCC instead. str0m's
/// `H264Packetizer` only walks for Annex B start codes, so AVCC bytes would
/// be silently dropped — leaving the viewer with `videoWidth=0` because the
/// first IDR cannot be decoded without SPS/PPS.
///
/// Detection is conservative: if the buffer already starts with an Annex B
/// start code (`00 00 00 01` or `00 00 01`), it's returned unchanged.
/// Otherwise we treat the first 4 bytes as a big-endian NALU length and walk
/// length-prefixed NALUs until the buffer is consumed, prefixing each with
/// `00 00 00 01`. Any trailing bytes are appended verbatim so we don't lose
/// data on edge cases the simple walk doesn't anticipate.
pub fn normalize_h264_extradata_to_annex_b(raw: &[u8]) -> Vec<u8> {
    if raw.is_empty() {
        return Vec::new();
    }
    if raw.starts_with(&[0, 0, 1]) {
        return raw.to_vec();
    }
    // Attempt AVCC walk: [4-byte len][NALU]+
    let mut out = Vec::with_capacity(raw.len() + 8);
    let mut i = 0;
    let mut walked = false;
    while i + 4 <= raw.len() {
        let len = u32::from_be_bytes([raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]);
        let len_usize = match usize::try_from(len) {
            Ok(v) => v,
            Err(_) => break,
        };
        // Sanity: NALU length must fit and must be non-zero. If it doesn't,
        // the buffer probably isn't AVCC and we bail out, returning the
        // original bytes so the caller can still attempt Annex B parsing.
        if len == 0 || i + 4 + len_usize > raw.len() {
            break;
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&raw[i + 4..i + 4 + len_usize]);
        i += 4 + len_usize;
        walked = true;
    }
    if walked && i == raw.len() {
        return out;
    }
    // Not AVCC, not Annex B (no start code at offset 0): return as-is so
    // str0m still sees the raw bytes — better than silently dropping them.
    raw.to_vec()
}

pub use crate::webrtc::video_toolbox_native::NativeVideoEncoder as VideoEncoder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_mixed_annex_b_start_codes() {
        let input = [0, 0, 1, 0x67, 1, 0, 0, 0, 1, 0x65, 2];
        let nalus = split_annex_b_nalus(&input);
        assert_eq!(nalus, vec![&[0x67, 1][..], &[0x65, 2][..]]);
    }

    #[test]
    fn detects_idr_keyframe() {
        assert!(is_keyframe(&[0, 0, 1, 0x65, 1]));
        assert!(!is_keyframe(&[0, 0, 1, 0x41, 1]));
    }

    #[test]
    fn converts_annex_b_to_avcc() {
        let avcc = annex_b_to_avcc(&[0, 0, 1, 0x67, 9, 0, 0, 1, 0x65, 1]).unwrap();
        assert_eq!(avcc, vec![0, 0, 0, 2, 0x67, 9, 0, 0, 0, 2, 0x65, 1]);
    }

    /// H.264 codec extradata may be either AVCC (length-prefixed) or Annex B
    /// (start-code-prefixed) depending on the encoder. str0m's H264Packetizer
    /// requires Annex B and walks the buffer looking for `00 00 00 01` /
    /// `00 00 01` start codes. If extradata arrives as AVCC and is forwarded
    /// verbatim, str0m never finds a start code and silently skips the entire
    /// keyframe, leaving the viewer with videoWidth=0.
    ///
    /// `normalize_h264_extradata_to_annex_b` must:
    ///   - Pass Annex B through unchanged.
    ///   - Convert AVCC (4-byte big-endian length prefix per NALU) to
    ///     Annex B with a `00 00 00 01` start code before every NALU.
    ///   - Return an empty Vec for empty input.
    #[test]
    fn normalizes_avcc_extradata_to_annex_b() {
        // Build a small SPS (NAL type 0x67) and PPS (NAL type 0x68) in
        // AVCC form: [4-byte SPS length][SPS][4-byte PPS length][PPS].
        let sps: &[u8] = &[0x67, 0x42, 0x00, 0x1e, 0xab, 0xcd];
        let pps: &[u8] = &[0x68, 0xce, 0x38, 0x80];
        let mut avcc = Vec::new();
        avcc.extend_from_slice(&(sps.len() as u32).to_be_bytes());
        avcc.extend_from_slice(sps);
        avcc.extend_from_slice(&(pps.len() as u32).to_be_bytes());
        avcc.extend_from_slice(pps);

        let out = normalize_h264_extradata_to_annex_b(&avcc);

        // Output must be Annex B: start code before SPS, start code before PPS.
        assert!(
            out.starts_with(&[0, 0, 0, 1]),
            "output must begin with Annex B start code, got {:02x?}",
            &out[..out.len().min(8)]
        );
        // Find both NALU bodies by locating the start codes.
        let nalus = split_annex_b_nalus(&out);
        assert_eq!(
            nalus.iter().map(|n| n.to_vec()).collect::<Vec<_>>(),
            vec![sps.to_vec(), pps.to_vec()],
            "AVCC extradata must be re-emitted as two Annex B NALUs"
        );
    }

    #[test]
    fn passes_annex_b_extradata_through_unchanged() {
        let annex_b = vec![0, 0, 0, 1, 0x67, 0x42, 0, 0, 0, 1, 0x68, 0xce];
        let out = normalize_h264_extradata_to_annex_b(&annex_b);
        assert_eq!(out, annex_b, "Annex B input must round-trip unchanged");
    }

    #[test]
    fn returns_empty_for_empty_extradata() {
        let out = normalize_h264_extradata_to_annex_b(&[]);
        assert!(out.is_empty(), "empty input must yield empty output");
    }

    /// Round-trip property: Annex B → AVCC → Annex B must equal the
    /// original Annex B. This pins down the inverse relationship and
    /// catches accidental data corruption in the conversion path.
    #[test]
    fn annex_b_avcc_annex_b_round_trip() {
        let original: &[u8] = &[
            0, 0, 0, 1, 0x67, 0x42, 0x00, 0x1e, 0xab, 0xcd, // SPS
            0, 0, 0, 1, 0x68, 0xce, 0x38, 0x80,             // PPS
            0, 0, 0, 1, 0x65, 0x88, 0x80, 0x40,             // IDR slice
        ];
        let avcc = annex_b_to_avcc(original).unwrap();
        let recovered = normalize_h264_extradata_to_annex_b(&avcc);
        assert_eq!(recovered, original, "round-trip must preserve bytes");
    }
}
