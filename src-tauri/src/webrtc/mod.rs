//! Screen capture -> H.264 -> RTP via str0m.

use image::RgbaImage;
use std::sync::Arc;
use std::time::Duration;

pub mod host;
pub mod video_toolbox;

pub use host::HostPeer;
pub use video_toolbox::{H264EncodedFrame, VideoEncoder};

#[derive(Debug, Clone, Copy)]
pub enum CaptureKind {
    Screen,
    Window,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureTarget {
    pub kind: CaptureKind,
    pub id: u32,
    /// 0.25 .. 1.0. Kept for compatibility with existing callers.
    pub quality: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptureSourceInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub width: u32,
    pub height: u32,
}

/// Enumerate available capture sources (monitors + windows).
pub fn enumerate_sources() -> Result<Vec<CaptureSourceInfo>, String> {
    #[cfg(target_os = "macos")]
    {
        use xcap::{Monitor, Window};
        let mut out = Vec::new();
        for (idx, m) in Monitor::all()
            .map_err(|e| e.to_string())?
            .into_iter()
            .enumerate()
        {
            out.push(CaptureSourceInfo {
                id: format!("screen:{idx}"),
                name: m.name().unwrap_or_else(|_| format!("Display {}", idx + 1)),
                kind: "screen".into(),
                width: m.width().unwrap_or(0),
                height: m.height().unwrap_or(0),
            });
        }
        if let Ok(windows) = Window::all() {
            for (idx, w) in windows.into_iter().enumerate() {
                let name = w.title().unwrap_or_else(|_| format!("Window {idx}"));
                if name.is_empty() {
                    continue;
                }
                out.push(CaptureSourceInfo {
                    id: format!("window:{idx}"),
                    name,
                    kind: "window".into(),
                    width: w.width().unwrap_or(0),
                    height: w.height().unwrap_or(0),
                });
            }
        }
        Ok(out)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}

pub struct CapturedFrame {
    pub rgba: RgbaImage,
    pub captured_at: std::time::Instant,
}

pub fn capture_one(target: &CaptureTarget) -> Result<CapturedFrame, String> {
    #[cfg(target_os = "macos")]
    {
        use xcap::{Monitor, Window};
        let img = match target.kind {
            CaptureKind::Screen => Monitor::all()
                .map_err(|e| e.to_string())?
                .into_iter()
                .nth(target.id as usize)
                .ok_or_else(|| format!("screen index {} out of range", target.id))?
                .capture_image()
                .map_err(|e| e.to_string())?,
            CaptureKind::Window => Window::all()
                .map_err(|e| e.to_string())?
                .into_iter()
                .nth(target.id as usize)
                .ok_or_else(|| format!("window index {} out of range", target.id))?
                .capture_image()
                .map_err(|e| e.to_string())?,
        };
        let (w, h) = (img.width(), img.height());
        let rgba = RgbaImage::from_raw(w, h, img.into_raw())
            .ok_or_else(|| "captured frame has wrong size".to_string())?;
        let max_dim: u32 = std::env::var("SCREENMIRROR_MAX_DIM")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1920);
        let rgba = if rgba.width() > max_dim || rgba.height() > max_dim {
            let scale = max_dim as f32 / rgba.width().max(rgba.height()) as f32;
            let nw = ((rgba.width() as f32) * scale).round().max(1.0) as u32;
            let nh = ((rgba.height() as f32) * scale).round().max(1.0) as u32;
            image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle)
        } else {
            rgba
        };
        Ok(CapturedFrame {
            rgba,
            captured_at: std::time::Instant::now(),
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("capture not implemented on this platform".into())
    }
}

pub type VideoFrameSink = Arc<dyn Fn(H264EncodedFrame) + Send + Sync + 'static>;

/// Capture RGBA frames, encode them with VideoToolbox, and deliver H.264 samples.
pub fn spawn_video_capture_loop(
    target: CaptureTarget,
    fps: u32,
    sink: VideoFrameSink,
) -> CaptureHandle {
    let interval = Duration::from_millis((1000 / fps.max(1)) as u64);
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    std::thread::spawn(move || {
        let mut encoder: Option<VideoEncoder> = None;
        while r.load(std::sync::atomic::Ordering::Relaxed) {
            match capture_one(&target) {
                Ok(frame) => {
                    let dimensions = (frame.rgba.width(), frame.rgba.height());
                    if encoder.is_none() {
                        match VideoEncoder::new(dimensions.0, dimensions.1, fps.max(1)) {
                            Ok(value) => encoder = Some(value),
                            Err(error) => {
                                tracing::warn!("video encoder initialization failed: {error}");
                                std::thread::sleep(interval);
                                continue;
                            }
                        }
                    }
                    if let Some(value) = encoder.as_ref() {
                        match value.encode(frame.rgba.as_raw()) {
                            Ok(encoded) => sink(encoded),
                            Err(error) => tracing::warn!("H.264 encode error: {error}"),
                        }
                    }
                }
                Err(error) => tracing::warn!("capture error: {error}"),
            }
            std::thread::sleep(interval);
        }
    });
    CaptureHandle { running }
}

pub struct CaptureHandle {
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl CaptureHandle {
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
