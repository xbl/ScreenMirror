//! ScreenCaptureKit capture interface.
//!
//! The concrete `SCStream` implementation is added in the next phase.  This
//! module deliberately keeps the frame contract independent from CoreVideo so
//! the WebRTC capture loop can select the source and fall back to xcap without
//! exposing platform-specific types across the rest of the crate.

use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};

use super::CaptureTarget;

/// A captured ScreenCaptureKit sample in a transport-friendly representation.
///
/// `bgra` is populated by the readback path.  `iosurface` is an opaque native
/// handle reserved for the zero-copy VideoToolbox path; it is intentionally a
/// `usize` here so non-macOS callers never need to link CoreVideo types.
#[derive(Debug)]
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
    frames: Receiver<ScreenKitFrame>,
    stopped: Arc<AtomicBool>,
}

impl ScreenKitCapture {
    pub(crate) fn from_parts(
        frames: Receiver<ScreenKitFrame>,
        stopped: Arc<AtomicBool>,
    ) -> Self {
        Self { frames, stopped }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<ScreenKitFrame, ScreenKitError> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(ScreenKitError::Stopped);
        }
        match self.frames.recv_timeout(timeout) {
            Ok(frame) => Ok(frame),
            Err(RecvTimeoutError::Timeout) => Err(ScreenKitError::Unavailable(
                "timed out waiting for a frame".into(),
            )),
            Err(RecvTimeoutError::Disconnected) => Err(ScreenKitError::Stopped),
        }
    }

    pub fn try_recv(&self) -> Result<ScreenKitFrame, ScreenKitError> {
        match self.frames.try_recv() {
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

#[cfg(target_os = "macos")]
pub fn start_screen_capture(
    _target: CaptureTarget,
    _fps: u32,
) -> Result<ScreenKitCapture, ScreenKitError> {
    // The SCStream wiring is implemented in Task 2.  Returning an error keeps
    // the caller on the existing xcap path until that implementation is ready.
    Err(ScreenKitError::Unavailable(
        "ScreenCaptureKit stream is not enabled yet".into(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn start_screen_capture(
    _target: CaptureTarget,
    _fps: u32,
) -> Result<ScreenKitCapture, ScreenKitError> {
    Err(ScreenKitError::UnsupportedPlatform)
}
