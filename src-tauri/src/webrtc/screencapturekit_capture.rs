//! ScreenCaptureKit capture interface.
//!
//! The concrete `SCStream` implementation is added in the next phase.  This
//! module deliberately keeps the frame contract independent from CoreVideo so
//! the WebRTC capture loop can select the source and fall back to xcap without
//! exposing platform-specific types across the rest of the crate.

use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use super::CaptureTarget;

/// A captured ScreenCaptureKit sample in a transport-friendly representation.
///
/// `iosurface` is retained for the direct VideoToolbox path. `bgra` is only
/// populated when ScreenCaptureKit cannot provide an IOSurface.
#[derive(Debug, Clone)]
pub struct ScreenKitFrame {
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub bgra: Option<Vec<u8>>,
    #[cfg(all(target_os = "macos", feature = "screenkit"))]
    pub iosurface: Option<apple_cf::iosurface::IOSurface>,
    #[cfg(not(all(target_os = "macos", feature = "screenkit")))]
    pub iosurface: Option<usize>,
    pub captured_at: Instant,
}

/// Errors returned while creating or operating a ScreenCaptureKit stream.
#[derive(Debug, thiserror::Error)]
pub enum ScreenKitError {
    #[error("ScreenCaptureKit is not supported on this platform")]
    UnsupportedPlatform,
    #[error("ScreenCaptureKit is unavailable: {0}")]
    Unavailable(String),
    #[error("ScreenCaptureKit stream stopped")]
    Stopped,
}

/// Handle for a running ScreenCaptureKit stream.
///
/// The frame receiver is bounded by the producer (capacity one in the concrete
/// implementation), allowing consumers to discard stale samples before encode.
pub struct ScreenKitCapture {
    frames: Arc<Mutex<Receiver<ScreenKitFrame>>>,
    stopped: Arc<AtomicBool>,
    #[cfg(all(target_os = "macos", feature = "screenkit"))]
    stream: Arc<Mutex<Option<screencapturekit::stream::sc_stream::SCStream>>>,
}

impl ScreenKitCapture {
    #[allow(dead_code)]
    pub(crate) fn from_parts(frames: Receiver<ScreenKitFrame>, stopped: Arc<AtomicBool>) -> Self {
        Self {
            frames: Arc::new(Mutex::new(frames)),
            stopped,
            #[cfg(all(target_os = "macos", feature = "screenkit"))]
            stream: Arc::new(Mutex::new(None)),
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<ScreenKitFrame, ScreenKitError> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(ScreenKitError::Stopped);
        }
        let frames = self.frames.lock().map_err(|_| ScreenKitError::Stopped)?;
        match frames.recv_timeout(timeout) {
            Ok(frame) => Ok(frame),
            Err(RecvTimeoutError::Timeout) => Err(ScreenKitError::Unavailable(
                "timed out waiting for a frame".into(),
            )),
            Err(RecvTimeoutError::Disconnected) => Err(ScreenKitError::Stopped),
        }
    }

    pub fn try_recv(&self) -> Result<ScreenKitFrame, ScreenKitError> {
        let frames = self.frames.lock().map_err(|_| ScreenKitError::Stopped)?;
        match frames.try_recv() {
            Ok(frame) => Ok(frame),
            Err(TryRecvError::Empty) => {
                Err(ScreenKitError::Unavailable("no frame available".into()))
            }
            Err(TryRecvError::Disconnected) => Err(ScreenKitError::Stopped),
        }
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }
}

#[cfg(all(target_os = "macos", feature = "screenkit"))]
impl Drop for ScreenKitCapture {
    fn drop(&mut self) {
        self.stop();
        if let Ok(mut stream) = self.stream.lock() {
            if let Some(stream) = stream.take() {
                let _ = stream.stop_capture();
            }
        }
    }
}

#[cfg(all(target_os = "macos", feature = "screenkit"))]
pub fn start_screen_capture(
    target: CaptureTarget,
    fps: u32,
) -> Result<ScreenKitCapture, ScreenKitError> {
    use screencapturekit::cm::{CMSampleBufferExt, CMSampleBufferSCExt, SCFrameStatus};
    use screencapturekit::shareable_content::SCShareableContent;
    use screencapturekit::stream::configuration::{PixelFormat, SCStreamConfiguration};
    use screencapturekit::stream::content_filter::SCContentFilter;
    use screencapturekit::stream::output_type::SCStreamOutputType;
    use screencapturekit::stream::sc_stream::SCStream;

    let content = SCShareableContent::get()
        .map_err(|error| ScreenKitError::Unavailable(error.to_string()))?;
    let (filter, source_width, source_height) = match target.kind {
        super::CaptureKind::Screen => {
            let displays = content.displays();
            let ids = displays.iter().map(|d| d.display_id().to_string()).collect::<Vec<_>>();
            let index = super::select_source_index(&ids, target.source_id.as_deref(), target.id, "display")
                .map_err(ScreenKitError::Unavailable)?;
            let display = displays.get(index).ok_or_else(|| ScreenKitError::Unavailable("display unavailable".into()))?;
            (
                SCContentFilter::create().with_display(display).with_excluding_windows(&[]).build(),
                display.width(),
                display.height(),
            )
        }
        super::CaptureKind::Window => {
            let windows = content.windows();
            let ids = windows.iter().map(|w| w.window_id().to_string()).collect::<Vec<_>>();
            let index = super::select_source_index(&ids, target.source_id.as_deref(), target.id, "window")
                .map_err(ScreenKitError::Unavailable)?;
            let window = windows.get(index).ok_or_else(|| ScreenKitError::Unavailable("window unavailable".into()))?;
            let frame = window.frame();
            (
                SCContentFilter::create().with_window(window).build(),
                frame.size.width.max(2.0) as u32,
                frame.size.height.max(2.0) as u32,
            )
        }
        _ => return Err(ScreenKitError::Unavailable("unsupported capture target".into())),
    };
    // Ask ScreenCaptureKit to scale before delivering BGRA. Capturing a full
    // Retina surface and resizing/copying it after delivery makes the cost
    // grow with the source pixel count even when the encoder target is 1920px.
    let (capture_width, capture_height) = super::capture_dimensions(
        source_width,
        source_height,
        super::profile_max_dim(target.quality),
    );
    tracing::info!(
        target = ?target.kind,
        source_width,
        source_height,
        capture_width,
        capture_height,
        fps,
        "ScreenCaptureKit configured source-scale capture"
    );
    let mut config = SCStreamConfiguration::new()
        .with_width(capture_width)
        .with_height(capture_height)
        .with_fps(fps.max(1))
        // Apple's ScreenCaptureKit requires a capture queue depth of at least
        // three on supported macOS versions. A depth of one can yield the
        // initial sample and then stop delivering samples under load.
        .with_queue_depth(3)
        .with_pixel_format(PixelFormat::BGRA);
    config.set_shows_cursor(true);

    let (sender, receiver) = std::sync::mpsc::sync_channel::<ScreenKitFrame>(1);
    let receiver_for_handler = Arc::new(Mutex::new(receiver));
    let receiver_for_handler_callback = receiver_for_handler.clone();
    let stopped = Arc::new(AtomicBool::new(false));
    let stopped_for_handler = stopped.clone();
    let handler = move |sample: screencapturekit::cm::CMSampleBuffer,
                        output_type: SCStreamOutputType| {
        if output_type != SCStreamOutputType::Screen || stopped_for_handler.load(Ordering::Acquire)
        {
            return;
        }
        if let Some(status) = sample.frame_status() {
            if !status.has_content() || matches!(status, SCFrameStatus::Idle) {
                return;
            }
        }
        let Some(pixel_buffer) = sample.image_buffer() else {
            return;
        };
        let width = pixel_buffer.width();
        let height = pixel_buffer.height();
        let bytes_per_row = pixel_buffer.bytes_per_row();
        if width == 0 || height == 0 || bytes_per_row < width.saturating_mul(4) {
            return;
        }
        // ScreenCaptureKit normally provides an IOSurface. Retain it and
        // hand it directly to VideoToolbox; copying full Retina frames here
        // is what previously made capture time scale into hundreds of ms.
        let iosurface = pixel_buffer.io_surface().map(|surface| surface.clone());
        let bgra = if iosurface.is_none() {
            let Ok(guard) = pixel_buffer.lock_read_only() else {
                return;
            };
            let base = guard.base_address();
            if base.is_null() {
                return;
            }
            let row_bytes = width.saturating_mul(4);
            let mut bytes = vec![0u8; row_bytes.saturating_mul(height)];
            for row in 0usize..height {
                // SAFETY: the lock keeps the base address valid for this copy.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        base.add(row.saturating_mul(bytes_per_row)),
                        bytes.as_mut_ptr().add(row.saturating_mul(row_bytes)),
                        row_bytes,
                    );
                }
            }
            Some(bytes)
        } else {
            None
        };
        let frame = ScreenKitFrame {
            width: width as u32,
            height: height as u32,
            bytes_per_row: bytes_per_row as u32,
            bgra,
            iosurface,
            // `display_time` is a mach-absolute timestamp; Instant::now() is
            // monotonic and is the timestamp consumed by the existing encoder age gate.
            captured_at: Instant::now(),
        };
        // Keep only the newest sample. If the bounded channel is full, remove
        // the queued stale frame before retrying the send.
        if let Err(std::sync::mpsc::TrySendError::Full(frame)) = sender.try_send(frame) {
            if let Ok(queued) = receiver_for_handler_callback.lock() {
                let _ = queued.try_recv();
            }
            let _ = sender.try_send(frame);
        }
    };
    let mut stream = SCStream::new(&filter, &config);
    stream
        .add_output_handler(handler, SCStreamOutputType::Screen)
        .ok_or_else(|| ScreenKitError::Unavailable("failed to register screen output".into()))?;
    stream
        .start_capture()
        .map_err(|error| ScreenKitError::Unavailable(error.to_string()))?;
    let capture = ScreenKitCapture {
        frames: receiver_for_handler,
        stopped,
        stream: Arc::new(Mutex::new(Some(stream))),
    };
    Ok(capture)
}

#[cfg(not(all(target_os = "macos", feature = "screenkit")))]
pub fn start_screen_capture(
    _target: CaptureTarget,
    _fps: u32,
) -> Result<ScreenKitCapture, ScreenKitError> {
    Err(ScreenKitError::UnsupportedPlatform)
}

#[cfg(all(test, not(all(target_os = "macos", feature = "screenkit"))))]
mod tests {
    use super::{start_screen_capture, ScreenKitError};
    use crate::webrtc::{CaptureKind, CaptureTarget};

    #[test]
    fn unavailable_platform_returns_a_recoverable_startup_error() {
        let result = start_screen_capture(
            CaptureTarget {
                kind: CaptureKind::Screen,
                id: u32::MAX,
                source_id: Some("missing-display".into()),
                quality: 0.75,
            },
            15,
        );

        assert!(matches!(result, Err(ScreenKitError::UnsupportedPlatform)));
    }
}
