//! ScreenCaptureKit capture interface.
//!
//! The concrete `SCStream` implementation is added in the next phase.  This
//! module deliberately keeps the frame contract independent from CoreVideo so
//! the WebRTC capture loop can select the source and fall back to xcap without
//! exposing platform-specific types across the rest of the crate.

use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};

use super::CaptureTarget;

/// A captured ScreenCaptureKit sample in a transport-friendly representation.
///
/// `bgra` is populated by the readback path.  `iosurface` is an opaque native
/// handle reserved for the zero-copy VideoToolbox path; it is intentionally a
/// `usize` here so non-macOS callers never need to link CoreVideo types.
#[derive(Debug, Clone)]
pub struct ScreenKitFrame {
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub bgra: Option<Vec<u8>>,
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
    pub(crate) fn from_parts(
        frames: Receiver<ScreenKitFrame>,
        stopped: Arc<AtomicBool>,
    ) -> Self {
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
            Err(TryRecvError::Empty) => Err(ScreenKitError::Unavailable(
                "no frame available".into(),
            )),
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

    if !matches!(target.kind, super::CaptureKind::Screen) {
        return Err(ScreenKitError::Unavailable(
            "ScreenCaptureKit currently supports display targets only".into(),
        ));
    }
    let content = SCShareableContent::get()
        .map_err(|error| ScreenKitError::Unavailable(error.to_string()))?;
    let displays = content.displays();
    let display = displays
        .get(target.id as usize)
        .ok_or_else(|| ScreenKitError::Unavailable(format!("display index {} out of range", target.id)))?;

    let filter = SCContentFilter::create()
        .with_display(display)
        .with_excluding_windows(&[])
        .build();
    let mut config = SCStreamConfiguration::new()
        .with_width(display.width())
        .with_height(display.height())
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
        if output_type != SCStreamOutputType::Screen || stopped_for_handler.load(Ordering::Acquire) {
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
        let Ok(guard) = pixel_buffer.lock_read_only() else {
            return;
        };
        let base = guard.base_address();
        if base.is_null() || width == 0 || height == 0 || bytes_per_row < width.saturating_mul(4) {
            return;
        }
        let row_bytes = width.saturating_mul(4);
        let mut bgra = vec![0u8; row_bytes.saturating_mul(height)];
        for row in 0usize..height {
            // SAFETY: CoreVideo keeps the base address valid while `guard` is alive;
            // each row is bounded by the reported stride and copied into our owned Vec.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    base.add(row.saturating_mul(bytes_per_row)),
                    bgra.as_mut_ptr().add(row.saturating_mul(row_bytes)),
                    row_bytes,
                );
            }
        }
        drop(guard);
        let frame = ScreenKitFrame {
            width: width as u32,
            height: height as u32,
            bytes_per_row: row_bytes as u32,
            bgra: Some(bgra),
            iosurface: pixel_buffer.io_surface().map(|surface| surface.as_ptr() as usize),
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
