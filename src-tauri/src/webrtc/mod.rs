//! Screen capture -> H.264 -> RTP via str0m.

use image::RgbaImage;
use std::sync::Arc;
use std::time::Duration;

pub mod ffi;
pub mod host;
pub mod video_toolbox;
pub mod video_toolbox_native;

pub use host::HostPeer;
pub use video_toolbox::{H264EncodedFrame, VideoEncoder};

#[derive(Debug, Clone, Copy)]
pub enum CaptureKind {
    Screen,
    Window,
    /// Synthetic moving gradient. Used by the E2E harness in environments
    /// without a real display so the host pipeline (encode + RTP + ICE + STAP-A)
    /// can be exercised end-to-end without xcap.
    TestPattern,
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
    capture_one_at(target, 0)
}

pub fn capture_one_at(target: &CaptureTarget, frame_index: u32) -> Result<CapturedFrame, String> {
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
            CaptureKind::TestPattern => render_test_pattern(target.id.wrapping_add(frame_index)),
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

/// Render a 320x180 RGBA frame with a moving radial gradient. Encoded by the
/// VideoToolbox encoder, the gradient provides enough spatial variation for the
/// H.264 codec to produce non-trivial keyframes. Deterministic per-frame-id so
/// the same input produces the same encoded output.
fn render_test_pattern(seed: u32) -> image::RgbaImage {
    // 320x180 animated pattern with LARGE flat-color quadrants so VideoToolbox
    // can encode keyframes quickly (~10-50ms instead of seconds), plus a
    // moving bright square so successive frames differ. Every quadrant has
    // a distinct, fully-saturated, non-black colour so the decoded frame
    // is visibly non-black under any compression setting.
    let w: u32 = 320;
    let h: u32 = 180;
    let mut buf = image::RgbaImage::new(w, h);
    // Background: four solid quadrants — bright and obviously non-black.
    for y in 0..h {
        for x in 0..w {
            let r: u8 = if x < w / 2 { 220 } else { 32 };
            let g: u8 = if y < h / 2 { 220 } else { 32 };
            let b: u8 = 96;
            buf.put_pixel(x, y, image::Rgba([r, g, b, 255]));
        }
    }
    // Overlay: a 40x40 white square that moves around so each frame differs
    // and the viewer always sees motion. seed is the frame index from
    // spawn_video_capture_loop, so the position animates over time.
    let t = (seed % 360) as f32;
    let cx = (w as f32 / 2.0 + 100.0 * (t * 0.01745).cos()) as i32;
    let cy = (h as f32 / 2.0 + 60.0 * (t * 0.01745).sin()) as i32;
    let sq = 40;
    for dy in 0..sq {
        for dx in 0..sq {
            let px = cx + dx - sq / 2;
            let py = cy + dy - sq / 2;
            if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                buf.put_pixel(px as u32, py as u32, image::Rgba([255, 255, 255, 255]));
            }
        }
    }
    // Border: 2px bright yellow so the frame edges are obviously visible.
    for x in 0..w {
        for t in 0..2 {
            buf.put_pixel(x, t, image::Rgba([255, 255, 0, 255]));
            buf.put_pixel(x, h - 1 - t, image::Rgba([255, 255, 0, 255]));
        }
    }
    for y in 0..h {
        for t in 0..2 {
            buf.put_pixel(t, y, image::Rgba([255, 255, 0, 255]));
            buf.put_pixel(w - 1 - t, y, image::Rgba([255, 255, 0, 255]));
        }
    }
    buf
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
    tracing::info!(
        "video capture loop spawned target={:?} fps={fps} interval={:?}",
        target,
        interval
    );
    std::thread::spawn(move || {
        let capture_started_at = std::time::Instant::now();
        let mut encoder_slot: Option<std::sync::Mutex<Option<VideoEncoder>>> = None;
        let mut frames: u32 = 0;
        while r.load(std::sync::atomic::Ordering::Relaxed) {
            let frame_index = frames;
            match capture_one_at(&target, frame_index) {
                Ok(frame) => {
                    let dimensions = (frame.rgba.width(), frame.rgba.height());
                    let pixels = (dimensions.0 as u64) * (dimensions.1 as u64);
                    let kbps = ((pixels * fps.max(1) as u64 * 6) / 1000 / 100)
                        .max(500)
                        .min(20000);
                    if encoder_slot.is_none() {
                        encoder_slot = Some(std::sync::Mutex::new(None));
                    }
                    let result = {
                        let mut guard = encoder_slot.as_mut().unwrap().lock().unwrap();
                        if guard.is_none() {
                            match VideoEncoder::new(
                                dimensions.0,
                                dimensions.1,
                                fps.max(1),
                                kbps as u32,
                            ) {
                                Ok(value) => *guard = Some(value),
                                Err(error) => {
                                    tracing::warn!("video encoder initialization failed: {error}");
                                    drop(guard);
                                    std::thread::sleep(interval);
                                    continue;
                                }
                            }
                        }
                        guard.as_mut().unwrap().encode(frame.rgba.as_raw())
                    };
                    match result {
                        Ok(encoded) => {
                            frames = frames.wrapping_add(1);
                            if frames <= 3 {
                                tracing::info!(
                                    "video capture: encoded frame #{} total_elapsed={:?} bytes={} keyframe={}",
                                    frames,
                                    capture_started_at.elapsed(),
                                    encoded.data.len(),
                                    encoded.keyframe,
                                );
                            }
                            if frames % 30 == 1 {
                                tracing::info!(
                                    "video capture: encoded frame #{} ({} bytes, keyframe={})",
                                    frames,
                                    encoded.data.len(),
                                    encoded.keyframe
                                );
                            }
                            sink(encoded);
                        }
                        Err(error) => tracing::warn!("H.264 encode error: {error}"),
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
