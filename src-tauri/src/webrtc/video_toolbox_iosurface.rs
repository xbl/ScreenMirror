//! Native VideoToolbox encoder boundary for ScreenCaptureKit frames.
//!
//! The current crate's generated FFI covers libavcodec only.  CoreVideo and
//! VideoToolbox bindings are intentionally kept out of that generated module
//! until their SDK/linking requirements can be validated on macOS.  This
//! module provides the stable boundary needed by the capture pipeline without
//! changing the existing FFmpeg-backed [`VideoEncoder`].

use crate::webrtc::screencapturekit_capture::ScreenKitFrame;
use crate::webrtc::video_toolbox::H264EncodedFrame;

/// Errors returned by the native IOSurface encoder boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IOSurfaceEncoderError {
    #[error("native VideoToolbox IOSurface encoding is not available in this build")]
    Unsupported,
    #[error("invalid encoder dimensions: {0}x{1}")]
    InvalidDimensions(u32, u32),
}

/// VideoToolbox encoder that consumes ScreenCaptureKit frames without an RGBA
/// conversion at its call site.
///
/// This is deliberately separate from `VideoEncoder` (the existing
/// libavcodec-backed implementation).  Until SDK bindings for
/// `VTCompressionSession` and `CVPixelBuffer` are validated on the supported
/// macOS toolchain, construction and encoding return `Unsupported` rather
/// than silently copying IOSurface data or risking an ABI mismatch.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct IOSurfaceVideoEncoder {
    width: u32,
    height: u32,
    fps: u32,
    bitrate_kbps: u32,
}

impl IOSurfaceVideoEncoder {
    /// Create an IOSurface encoder configuration.
    ///
    /// The dimensions are validated before reporting unsupported so callers
    /// can distinguish malformed capture settings from a missing native
    /// backend during migration.
    pub fn new(
        width: u32,
        height: u32,
        _fps: u32,
        _bitrate_kbps: u32,
    ) -> Result<Self, IOSurfaceEncoderError> {
        if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
            return Err(IOSurfaceEncoderError::InvalidDimensions(width, height));
        }
        Err(IOSurfaceEncoderError::Unsupported)
    }

    /// Encode one ScreenCaptureKit frame.
    ///
    /// This method is the hand-off point for the eventual
    /// `VTCompressionSessionEncodeFrame` implementation.  It intentionally
    /// does not read `frame.bgra`, preserving the no-copy contract for the
    /// native path and making accidental fallback copies visible to callers.
    pub fn encode(
        &mut self,
        _frame: ScreenKitFrame,
    ) -> Result<H264EncodedFrame, IOSurfaceEncoderError> {
        let _ = (self.width, self.height, self.fps, self.bitrate_kbps);
        Err(IOSurfaceEncoderError::Unsupported)
    }

    /// Flush pending native output.  There is no native session while this
    /// backend is unsupported, so flushing has no side effects.
    pub fn flush(&mut self) -> Result<(), IOSurfaceEncoderError> {
        Err(IOSurfaceEncoderError::Unsupported)
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Compile-time capability probe used by the capture loop's future backend
/// selection.  It remains false until the SDK-backed implementation lands.
pub const fn is_available() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_odd_dimensions_before_backend_probe() {
        assert_eq!(
            IOSurfaceVideoEncoder::new(321, 240, 30, 1),
            Err(IOSurfaceEncoderError::InvalidDimensions(321, 240))
        );
    }

    #[test]
    fn reports_unsupported_without_changing_existing_encoder() {
        assert!(!is_available());
        assert_eq!(
            IOSurfaceVideoEncoder::new(320, 240, 30, 500),
            Err(IOSurfaceEncoderError::Unsupported)
        );
    }
}
