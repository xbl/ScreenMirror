//! Native VideoToolbox encoder for ScreenCaptureKit frames.
//!
//! The implementation is feature-gated so the normal xcap build keeps its
//! existing dependency and linker surface.  On macOS with `screenkit`, BGRA
//! frames are submitted from their ScreenCaptureKit IOSurface to a persistent
//! hardware `VTCompressionSession` through the `videotoolbox` crate.

use crate::webrtc::screencapturekit_capture::ScreenKitFrame;
use crate::webrtc::video_toolbox::H264EncodedFrame;

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
    use crate::webrtc::video_toolbox::avcc_to_annex_b;
    use apple_cf::iosurface::{IOSurface, IOSurfaceLockOptions};
    use videotoolbox::compression::CompressionSession;
    use videotoolbox::session::Codec;

    fn build_session(
        width: u32,
        height: u32,
        fps: u32,
        bitrate_kbps: u32,
    ) -> Result<CompressionSession, IOSurfaceEncoderError> {
        let session = CompressionSession::builder(width as i32, height as i32, Codec::H264)
            .with_real_time(true)
            .with_allow_frame_reordering(false)
            .with_profile_level(videotoolbox::compression::ProfileLevel::H264HighAutoLevel)
            .with_average_bit_rate((bitrate_kbps.max(1) as i32).saturating_mul(1000))
            .with_expected_frame_rate(fps.max(1) as f64)
            .with_max_keyframe_interval(fps.max(1) as i32)
            .build()
            .map_err(|e| IOSurfaceEncoderError::Native(e.to_string()))?;

        // Configure zero frame delay and CABAC entropy coding
        unsafe {
            let delay_count: i32 = 0;
            let num = videotoolbox::ffi::CFNumberCreate(
                videotoolbox::ffi::kCFAllocatorDefault,
                videotoolbox::ffi::kCFNumberSInt32Type,
                core::ptr::from_ref(&delay_count).cast(),
            );
            if !num.is_null() {
                let res = session.set_property(
                    videotoolbox::ffi::kVTCompressionPropertyKey_MaxFrameDelayCount,
                    num.cast(),
                );
                videotoolbox::ffi::CFRelease(num.cast());
                if let Err(e) = res {
                    // kVTPropertyNotSupportedErr (-12900) is returned when the hardware
                    // encoder enforces real-time zero delay via RealTime + AllowFrameReordering=false.
                    tracing::debug!("kVTCompressionPropertyKey_MaxFrameDelayCount not supported or failed: {e}");
                }
            }

            // CABAC entropy mode produces cleaner text and saves 10-15% bitrate
            let _ = session.set_property(
                videotoolbox::ffi::kVTCompressionPropertyKey_H264EntropyMode,
                videotoolbox::ffi::kVTH264EntropyMode_CABAC.cast(),
            );
        }

        Ok(session)
    }

    fn coremedia_sample_is_sync(sample: &apple_cf::cm::CMSampleBuffer) -> Option<bool> {
        // kCMSampleAttachmentKey_NotSync is absent for a sync sample. Read
        // the attachment directly because apple-cf intentionally exposes only
        // the framework-neutral CMSampleBuffer surface.
        unsafe {
            let attachments =
                apple_cf::raw::CMSampleBufferGetSampleAttachmentsArray(sample.as_ptr().cast(), 0);
            if attachments.is_null() || apple_cf::raw::CFArrayGetCount(attachments) == 0 {
                return None;
            }
            let dictionary = apple_cf::raw::CFArrayGetValueAtIndex(attachments, 0);
            if dictionary.is_null() {
                return None;
            }
            let mut not_sync = std::ptr::null();
            let present = apple_cf::raw::CFDictionaryGetValueIfPresent(
                dictionary.cast(),
                apple_cf::raw::kCMSampleAttachmentKey_NotSync.cast(),
                &mut not_sync,
            );
            if present == 0 {
                return Some(true);
            }
            if not_sync.is_null()
                || apple_cf::raw::CFGetTypeID(not_sync) != apple_cf::raw::CFBooleanGetTypeID()
            {
                return None;
            }
            Some(apple_cf::raw::CFBooleanGetValue(not_sync.cast()) == 0)
        }
    }

    #[derive(Debug)]
    pub struct Encoder {
        width: u32,
        height: u32,
        fps: u32,
        bitrate_kbps: u32,
        session: CompressionSession,
        fallback_surface: Option<IOSurface>,
        pts: i64,
        /// PTS clock origin, shared across session rebuilds so presentation
        /// timestamps stay monotonic in wall-clock time. Capture is
        /// change-driven, so a uniform per-frame PTS would distort playback
        /// rate after any static gap.
        pts_origin: std::time::Instant,
        last_keyframe_time: std::time::Instant,
        sps_pps: Vec<u8>,
    }

    impl Encoder {
        pub fn new(
            width: u32,
            height: u32,
            fps: u32,
            bitrate_kbps: u32,
        ) -> Result<Self, IOSurfaceEncoderError> {
            if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
                return Err(IOSurfaceEncoderError::InvalidDimensions(width, height));
            }
            let session = build_session(width, height, fps, bitrate_kbps)?;
            Ok(Self {
                width,
                height,
                fps: fps.max(1),
                bitrate_kbps,
                session,
                fallback_surface: None,
                pts: 0,
                pts_origin: std::time::Instant::now(),
                last_keyframe_time: std::time::Instant::now(),
                sps_pps: Vec::new(),
            })
        }

        /// Request that the next encoded frame be an IDR. The sync
        /// `CompressionSession::encode` API cannot pass per-frame
        /// properties (`kVTEncodeFrameOptionKey_ForceKeyFrame`), so rebuild
        /// the session instead — a fresh session always starts with an IDR.
        /// Rebuilding destroys and reallocates the hardware encoder (30-50ms
        /// stall), so throttle forced rebuilds to at most once per 1.0 second.
        /// The natural GOP interval already guarantees an IDR every 1.0 second.
        pub fn force_keyframe(&mut self) {
            let now = std::time::Instant::now();
            if now.duration_since(self.last_keyframe_time) < std::time::Duration::from_millis(1000) {
                tracing::debug!("throttling forced keyframe rebuild; natural GOP will refresh shortly");
                return;
            }
            match build_session(self.width, self.height, self.fps, self.bitrate_kbps) {
                Ok(session) => {
                    self.session = session;
                    self.last_keyframe_time = now;
                    // The new session has new SPS/PPS; re-extract lazily.
                    self.sps_pps.clear();
                }
                Err(error) => {
                    tracing::warn!("failed to rebuild session for forced IDR: {error}");
                }
            }
        }

        pub fn encode(
            &mut self,
            frame: &ScreenKitFrame,
        ) -> Result<H264EncodedFrame, IOSurfaceEncoderError> {
            let expected_row = self.width as usize * 4;
            if frame.width != self.width
                || frame.height != self.height
                || frame.bytes_per_row < expected_row as u32
            {
                return Err(IOSurfaceEncoderError::Native(format!(
                    "frame dimensions {}x{} do not match encoder {}x{}",
                    frame.width, frame.height, self.width, self.height
                )));
            }
            let pts_ms = frame.captured_at.duration_since(self.pts_origin).as_millis() as i64;
            self.pts = self.pts.saturating_add(1).max(pts_ms);
            let presentation_time = (self.pts, 1000);
            let encoded = if let Some(surface) = frame.iosurface.as_ref() {
                self.session.encode(surface, presentation_time)
            } else {
                let bgra = frame
                    .bgra
                    .as_deref()
                    .ok_or(IOSurfaceEncoderError::MissingFrameData)?;
                let surface = self.fallback_surface.get_or_insert_with(|| {
                    IOSurface::create(
                        self.width as usize,
                        self.height as usize,
                        u32::from_be_bytes(*b"BGRA"),
                        4,
                    )
                    .expect("validated IOSurface dimensions")
                });
                let mut guard = surface
                    .lock(IOSurfaceLockOptions::from_bits(0))
                    .map_err(|e| {
                        IOSurfaceEncoderError::Native(format!("IOSurface lock failed: {e}"))
                    })?;
                let dst_stride = guard.bytes_per_row();
                let dst = guard.as_slice_mut().ok_or_else(|| {
                    IOSurfaceEncoderError::Native("IOSurface is not writable".into())
                })?;
                let src_stride = frame.bytes_per_row as usize;
                for row in 0..self.height as usize {
                    let src_start = row * src_stride;
                    let dst_start = row * dst_stride;
                    let src_end = src_start + expected_row;
                    let dst_end = dst_start + expected_row;
                    if src_end > bgra.len() || dst_end > dst.len() {
                        return Err(IOSurfaceEncoderError::Native(
                            "BGRA frame buffer is truncated".into(),
                        ));
                    }
                    dst[dst_start..dst_end].copy_from_slice(&bgra[src_start..src_end]);
                }
                drop(guard);
                self.session.encode(surface, presentation_time)
            }
            .map_err(|e| IOSurfaceEncoderError::Native(e.to_string()))?;
            if self.sps_pps.is_empty() {
                if let Some(sample) = encoded.cm_sample_buffer() {
                    if let Some(format) = sample.format_description() {
                        let mut index = 0usize;
                        loop {
                            let mut pointer = std::ptr::null();
                            let mut size = 0usize;
                            let mut count = 0usize;
                            let mut header = 0;
                            let status = unsafe {
                                apple_cf::raw::CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                                    format.as_ptr().cast(),
                                    index,
                                    &mut pointer,
                                    &mut size,
                                    &mut count,
                                    &mut header,
                                )
                            };
                            if status != 0 || pointer.is_null() || size == 0 {
                                break;
                            }
                            self.sps_pps.extend_from_slice(&[0, 0, 0, 1]);
                            self.sps_pps.extend_from_slice(unsafe {
                                std::slice::from_raw_parts(pointer, size)
                            });
                            index += 1;
                            if index >= count {
                                break;
                            }
                        }
                    }
                }
            }
            let coremedia_sync = encoded
                .cm_sample_buffer()
                .and_then(coremedia_sample_is_sync);
            let mut data = avcc_to_annex_b(&encoded.data).unwrap_or(encoded.data);
            let keyframe = crate::webrtc::video_toolbox::keyframe_from_markers(
                crate::webrtc::video_toolbox::is_keyframe(&data),
                coremedia_sync,
            );
            if keyframe {
                self.last_keyframe_time = std::time::Instant::now();
                let has_sps = crate::webrtc::video_toolbox::split_annex_b_nalus(&data)
                    .iter()
                    .any(|nal| nal.first().map(|byte| byte & 0x1f) == Some(7));
                let has_pps = crate::webrtc::video_toolbox::split_annex_b_nalus(&data)
                    .iter()
                    .any(|nal| nal.first().map(|byte| byte & 0x1f) == Some(8));
                if !has_sps || !has_pps {
                    if self.sps_pps.is_empty() {
                        return Err(IOSurfaceEncoderError::Native(
                            "VideoToolbox format description has no SPS/PPS".into(),
                        ));
                    }
                    let mut with_params = self.sps_pps.clone();
                    with_params.extend_from_slice(&data);
                    data = with_params;
                }
            }
            Ok(H264EncodedFrame {
                data,
                keyframe,
                captured_at: frame.captured_at,
            })
        }

        pub fn flush(&mut self) -> Result<(), IOSurfaceEncoderError> {
            Ok(())
        }
        pub fn dimensions(&self) -> (u32, u32) {
            (self.width, self.height)
        }
    }

    // No `available()` probe here: the videotoolbox crate exposes no cheap
    // synchronous way to verify VideoToolbox will actually accept frames, so
    // an unconditional `true` would be misleading. Callers gate on the
    // compile-time `is_available()` and handle runtime encode errors.
}

#[derive(Debug)]
pub struct IOSurfaceVideoEncoder {
    #[cfg(all(target_os = "macos", feature = "screenkit"))]
    inner: native::Encoder,
    #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
    dimensions: (u32, u32),
}

impl IOSurfaceVideoEncoder {
    pub fn new(
        width: u32,
        height: u32,
        fps: u32,
        bitrate_kbps: u32,
    ) -> Result<Self, IOSurfaceEncoderError> {
        if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
            return Err(IOSurfaceEncoderError::InvalidDimensions(width, height));
        }
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        {
            return native::Encoder::new(width, height, fps, bitrate_kbps)
                .map(|inner| Self { inner });
        }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        {
            let _ = (fps, bitrate_kbps);
            Err(IOSurfaceEncoderError::Unsupported)
        }
    }

    pub fn encode(
        &mut self,
        frame: &ScreenKitFrame,
    ) -> Result<H264EncodedFrame, IOSurfaceEncoderError> {
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        {
            self.inner.encode(frame)
        }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        {
            let _ = frame;
            Err(IOSurfaceEncoderError::Unsupported)
        }
    }

    pub fn force_keyframe(&mut self) {
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        {
            self.inner.force_keyframe()
        }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        {}
    }

    pub fn flush(&mut self) -> Result<(), IOSurfaceEncoderError> {
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        {
            self.inner.flush()
        }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        {
            Err(IOSurfaceEncoderError::Unsupported)
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        {
            self.inner.dimensions()
        }
        #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
        {
            self.dimensions
        }
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
        assert!(matches!(
            IOSurfaceVideoEncoder::new(321, 240, 30, 1),
            Err(IOSurfaceEncoderError::InvalidDimensions(321, 240))
        ));
    }
    #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
    #[test]
    fn default_build_reports_unsupported() {
        assert!(!is_available());
        assert!(matches!(
            IOSurfaceVideoEncoder::new(320, 240, 30, 500),
            Err(IOSurfaceEncoderError::Unsupported)
        ));
    }
    #[cfg(all(target_os = "macos", feature = "screenkit"))]
    #[test]
    fn screenkit_build_reports_available() {
        assert!(is_available());
        let encoder = IOSurfaceVideoEncoder::new(320, 240, 30, 500);
        assert!(encoder.is_ok(), "failed to create encoder: {:?}", encoder.err());
    }
}
