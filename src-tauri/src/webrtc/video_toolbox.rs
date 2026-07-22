//! H.264 encoding helpers and the macOS VideoToolbox backend.
//!
//! The backend uses the system FFmpeg build only as a thin VideoToolbox
//! adapter. It emits Annex-B H.264 access units suitable for str0m's H.264
//! packetizer. No image or data-channel framing is involved.

use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct H264EncodedFrame {
    pub data: Vec<u8>,
    pub keyframe: bool,
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

pub struct VideoEncoder {
    width: u32,
    height: u32,
    fps: u32,
}

impl VideoEncoder {
    pub fn new(width: u32, height: u32, fps: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("encoder dimensions must be non-zero".into());
        }
        if fps == 0 {
            return Err("encoder FPS must be non-zero".into());
        }
        #[cfg(target_os = "macos")]
        {
            let output = Command::new("ffmpeg")
                .args(["-hide_banner", "-encoders"])
                .output()
                .map_err(|e| format!("cannot execute ffmpeg: {e}"))?;
            let text = String::from_utf8_lossy(&output.stdout);
            if !text.contains("h264_videotoolbox") {
                return Err("ffmpeg does not provide h264_videotoolbox".into());
            }
            Ok(Self { width, height, fps })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (width, height, fps);
            Err("H.264 VideoToolbox encoding is only supported on macOS".into())
        }
    }

    pub fn encode(&self, rgba: &[u8]) -> Result<H264EncodedFrame, String> {
        let expected = self.width as usize * self.height as usize * 4;
        if rgba.len() != expected {
            return Err(format!(
                "expected {expected} RGBA bytes, got {}",
                rgba.len()
            ));
        }
        #[cfg(target_os = "macos")]
        {
            let size = format!("{}x{}", self.width, self.height);
            let mut child = Command::new("ffmpeg")
                .args([
                    "-loglevel",
                    "error",
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgba",
                    "-s",
                    &size,
                    "-r",
                    &self.fps.to_string(),
                    "-i",
                    "-",
                    "-frames:v",
                    "1",
                    "-c:v",
                    "h264_videotoolbox",
                    "-profile:v",
                    "high",
                    "-tune",
                    "zerolatency",
                    "-f",
                    "h264",
                    "-",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("cannot start ffmpeg: {e}"))?;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(rgba)
                .map_err(|e| format!("ffmpeg stdin: {e}"))?;
            let output = child
                .wait_with_output()
                .map_err(|e| format!("ffmpeg wait: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "ffmpeg encode failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            if output.stdout.is_empty() {
                return Err("ffmpeg returned an empty H.264 sample".into());
            }
            Ok(H264EncodedFrame {
                keyframe: is_keyframe(&output.stdout),
                data: output.stdout,
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = rgba;
            Err("H.264 VideoToolbox encoding is only supported on macOS".into())
        }
    }
}

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
}
