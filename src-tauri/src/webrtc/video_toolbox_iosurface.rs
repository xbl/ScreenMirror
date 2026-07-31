//! Native VideoToolbox encoder for ScreenCaptureKit frames.
//!
//! The implementation is feature-gated so the normal xcap build keeps its
//! existing dependency and linker surface.  On macOS with `screenkit`, BGRA
//! frames are copied into an IOSurface and submitted to a persistent hardware
//! `VTCompressionSession` through the `videotoolbox` crate.

use crate::webrtc::screencapturekit_capture::ScreenKitFrame;
use crate::webrtc::video_toolbox::{annex_b_to_avcc, H264EncodedFrame};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IOSurfaceEncoderError {
    #[error("native VideoToolbox IOSurface encoding is not available in this build")]
    Unsupported,
    #[error("invalid encoder dimensions: {0}x{1}")]
    InvalidDimensions(u32, u32),
    #[error("native VideoToolbox error: {0}")]
    Native(String),
    #[error("frame does not contain BGRA data")]
    MissingFrameData,
}

#[cfg(all(target_os = "macos", feature = "screenkit"))]
mod native {
    use super::*;
    use apple_cf::iosurface::{IOSurface, IOSurfaceLockOptions};
    use videotoolbox::compression::CompressionSession;
    use videotoolbox::session::Codec;

    #[derive(Debug)]
    pub struct Encoder {
        width: u32,
        height: u32,
        fps: u32,
        session: CompressionSession,
        pts: i64,
    }

    impl Encoder {
        pub fn new(width: u32, height: u32, fps: u32, bitrate_kbps: u32) -> Result<Self, IOSurfaceEncoderError> {
            if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
                return Err(IOSurfaceEncoderError::InvalidDimensions(width, height));
            }
            let session = CompressionSession::builder(width as i32, height as i32, Codec::H264)
                .with_real_time(true)
                .with_allow_frame_reordering(false)
                .with_average_bit_rate((bitrate_kbps.max(1) as i32).saturating_mul(1000))
                .with_expected_frame_rate(fps.max(1) as f64)
                .with_max_keyframe_interval((fps.max(1) * 2) as i32)
                .build()
                .map_err(|e| IOSurfaceEncoderError::Native(e.to_string()))?;
            Ok(Self { width, height, fps: fps.max(1), session, pts: 0 })
        }

        pub fn encode(&mut self, frame: ScreenKitFrame) -> Result<H264EncodedFrame, IOSurfaceEncoderError> {
            let bgra = frame.bgra.as_deref().ok_or(IOSurfaceEncoderError::MissingFrameData)?;
            let surface = IOSurface::create(self.width as usize, self.height as usize, u32::from_be_bytes(*b"BGRA"), 4)
                .ok_or_else(|| IOSurfaceEncoderError::Native("IOSurface::create failed".into()))?;
            let expected_row = self.width as usize * 4;
            if frame.width != self.width || frame.height != self.height || frame.bytes_per_row < expected_row as u32 {
                return Err(IOSurfaceEncoderError::Native(format!("frame dimensions {}x{} do not match encoder {}x{}", frame.width, frame.height, self.width, self.height)));
            }
            let mut guard = surface.lock(IOSurfaceLockOptions::from_bits(0)).map_err(|e| IOSurfaceEncoderError::Native(format!("IOSurface lock failed: {e}")))?;
            let dst_stride = guard.bytes_per_row();
            let dst = guard.as_slice_mut().ok_or_else(|| IOSurfaceEncoderError::Native("IOSurface is not writable".into()))?;
            let src_stride = frame.bytes_per_row as usize;
            for row in 0..self.height as usize {
                let src_start = row * src_stride;
                let dst_start = row * dst_stride;
                let src_end = src_start + expected_row;
                let dst_end = dst_start + expected_row;
                if src_end > bgra.len() || dst_end > dst.len() { return Err(IOSurfaceEncoderError::Native("BGRA frame buffer is truncated".into())); }
                dst[dst_start..dst_end].copy_from_slice(&bgra[src_start..src_end]);
            }
            drop(guard);
            let encoded = self.session.encode(&surface, (self.pts, self.fps as i32)).map_err(|e| IOSurfaceEncoderError::Native(e.to_string()))?;
            self.pts = self.pts.saturating_add(1);
            let data = annex_b_to_avcc(&encoded.data).unwrap_or(encoded.data);
            let keyframe = crate::webrtc::video_toolbox::is_keyframe(&data);
            Ok(H264EncodedFrame { data, keyframe, captured_at: frame.captured_at })
        }

        pub fn flush(&mut self) -> Result<(), IOSurfaceEncoderError> { Ok(()) }
        pub fn dimensions(&self) -> (u32, u32) { (self.width, self.height) }
    }

    pub fn available() -> bool { true }
}

#[derive(Debug)]
pub struct IOSurfaceVideoEncoder {
    #[cfg(all(target_os = "macos", feature = "screenkit"))]
    inner: native::Encoder,
    #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
    dimensions: (u32, u32),
}

impl IOSurfaceVideoEncoder {
    pub fn new(width: u32, height: u32, fps: u32, bitrate_kbps: u32) -> Result<Self, IOSurfaceEncoderError> {
        if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
            return Err(IOSurfaceEncoderError::InvalidDimensions(width, height));
        }
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        { return native::Encoder::new(width, height, fps, bitrate_kbps).map(|inner| Self { inner }); }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        { let _ = (fps, bitrate_kbps); Err(IOSurfaceEncoderError::Unsupported) }
    }

    pub fn encode(&mut self, frame: ScreenKitFrame) -> Result<H264EncodedFrame, IOSurfaceEncoderError> {
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        { self.inner.encode(frame) }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        { let _ = frame; Err(IOSurfaceEncoderError::Unsupported) }
    }

    pub fn flush(&mut self) -> Result<(), IOSurfaceEncoderError> {
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        { self.inner.flush() }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        { Err(IOSurfaceEncoderError::Unsupported) }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        { self.inner.dimensions() }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        { self.dimensions }
    }
}

pub const fn is_available() -> bool {
    cfg!(all(target_os = "macos", feature = "screenkit"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_odd_dimensions() {
        assert_eq!(IOSurfaceVideoEncoder::new(321, 240, 30, 1), Err(IOSurfaceEncoderError::InvalidDimensions(321, 240)));
    }
    #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
    #[test]
    fn default_build_reports_unsupported() {
        assert!(!is_available());
        assert_eq!(IOSurfaceVideoEncoder::new(320, 240, 30, 500), Err(IOSurfaceEncoderError::Unsupported));
    }
}
