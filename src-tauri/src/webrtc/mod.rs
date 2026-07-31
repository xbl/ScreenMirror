//! Screen capture -> H.264 -> RTP via str0m.

use image::RgbaImage;
use std::sync::mpsc::RecvTimeoutError;
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

fn normalize_encoder_dimensions(width: u32, height: u32) -> (u32, u32) {
    (width & !1, height & !1)
}

fn resize_rgba_nearest(rgba: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let src_width = rgba.width();
    let src_height = rgba.height();
    let src = rgba.as_raw();
    let mut dst = vec![0u8; (width as usize) * (height as usize) * 4];

    // Precompute the horizontal lookup once. The old implementation did a
    // 64-bit division for every pixel, which made a Retina-sized capture take
    // roughly a second before it could reach the encoder.
    let x_map: Vec<u32> = (0..width)
        .map(|x| ((x as u64) * (src_width as u64) / (width as u64)) as u32)
        .collect();
    for y in 0..height {
        let sy = ((y as u64) * (src_height as u64) / (height as u64)) as u32;
        let src_row = (sy as usize) * (src_width as usize) * 4;
        let dst_row = (y as usize) * (width as usize) * 4;
        for x in 0..width {
            let src_offset = src_row + (x_map[x as usize] as usize) * 4;
            let dst_offset = dst_row + (x as usize) * 4;
            dst[dst_offset..dst_offset + 4]
                .copy_from_slice(&src[src_offset..src_offset + 4]);
        }
    }
    RgbaImage::from_raw(width, height, dst).expect("nearest resize dimensions match buffer")
}

fn normalize_captured_rgba_with_max_dim(rgba: RgbaImage, max_dim: u32) -> RgbaImage {
    let max_dim = max_dim.max(2);
    let rgba = if rgba.width() > max_dim || rgba.height() > max_dim {
        let scale = max_dim as f32 / rgba.width().max(rgba.height()) as f32;
        let nw = ((rgba.width() as f32) * scale).round().max(1.0) as u32;
        let nh = ((rgba.height() as f32) * scale).round().max(1.0) as u32;
        resize_rgba_nearest(&rgba, nw, nh)
    } else {
        rgba
    };
    let (encoder_width, encoder_height) = normalize_encoder_dimensions(rgba.width(), rgba.height());
    if (encoder_width, encoder_height) != (rgba.width(), rgba.height()) {
        image::imageops::crop_imm(&rgba, 0, 0, encoder_width, encoder_height).to_image()
    } else {
        rgba
    }
}

fn profile_max_dim(quality: f32) -> u32 {
    match quality {
        q if q >= 0.9 => 3840,
        // High uses 1920px with a decode-friendly frame rate. Ultra remains
        // an explicit opt-in for devices that can sustain larger frames.
        q if q >= 0.65 => 1920,
        _ => 1920,
    }
}

pub fn profile_fps(quality: f32) -> u32 {
    if quality >= 0.9 {
        15
    } else if quality >= 0.65 {
        20
    } else {
        30
    }
}

fn normalize_captured_rgba(rgba: RgbaImage, quality: f32) -> RgbaImage {
    let max_dim: u32 = std::env::var("SCREENMIRROR_MAX_DIM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| profile_max_dim(quality));
    normalize_captured_rgba_with_max_dim(rgba, max_dim)
}

fn capture_bitrate_kbps(width: u32, height: u32, fps: u32, quality: f32) -> u32 {
    let pixels = u64::from(width) * u64::from(height);
    let quality = quality.clamp(0.25, 1.0);
    // Screen text needs more bits than camera video. At quality=1 this is
    // about 3.7 Mbps for 1080p15, while retaining the existing frame rate.
    let kbps = ((pixels as f64 * fps.max(1) as f64 * 12.0 * quality as f64) / 100_000.0)
        .round() as u64;
    kbps.clamp(1_200, 20_000) as u32
}

fn captured_frame_from_rgba(rgba: RgbaImage, quality: f32) -> CapturedFrame {
    CapturedFrame {
        rgba: normalize_captured_rgba(rgba, quality),
        captured_at: std::time::Instant::now(),
    }
}

pub fn capture_one(target: &CaptureTarget) -> Result<CapturedFrame, String> {
    capture_one_at(target, 0)
}

pub fn capture_one_at(target: &CaptureTarget, frame_index: u32) -> Result<CapturedFrame, String> {
    #[cfg(target_os = "macos")]
    {
        capture_one_at_with_monitor(target, frame_index, None)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (target, frame_index);
        Err("capture not implemented on this platform".into())
    }
}

#[cfg(target_os = "macos")]
fn capture_one_at_with_monitor(
    target: &CaptureTarget,
    frame_index: u32,
    cached_monitor: Option<&xcap::Monitor>,
) -> Result<CapturedFrame, String> {
    use xcap::{Monitor, Window};
    let img = match target.kind {
        CaptureKind::Screen => {
            let monitor = match cached_monitor {
                Some(monitor) => monitor.clone(),
                None => Monitor::all()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .nth(target.id as usize)
                    .ok_or_else(|| format!("screen index {} out of range", target.id))?,
            };
            monitor.capture_image()
        }
        CaptureKind::Window => Window::all()
            .map_err(|e| e.to_string())?
            .into_iter()
            .nth(target.id as usize)
            .ok_or_else(|| format!("window index {} out of range", target.id))?
            .capture_image(),
        CaptureKind::TestPattern => Ok(render_test_pattern(target.id.wrapping_add(frame_index))),
    }
    .map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    let rgba = RgbaImage::from_raw(w, h, img.into_raw())
        .ok_or_else(|| "captured frame has wrong size".to_string())?;
    Ok(captured_frame_from_rgba(rgba, target.quality))
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
        #[cfg(target_os = "macos")]
        let cached_monitor = if matches!(target.kind, CaptureKind::Screen) {
            xcap::Monitor::all()
                .map_err(|e| e.to_string())
                .and_then(|monitors| {
                    monitors
                        .into_iter()
                        .nth(target.id as usize)
                        .ok_or_else(|| format!("screen index {} out of range", target.id))
                })
                .ok()
        } else {
            None
        };
        #[cfg(target_os = "macos")]
        let mut screen_capture = {
            // On some macOS versions video_recorder() is change-driven and
            // does not emit an initial frame. The loop seeds one frame with
            // capture_image() before using the recorder for ongoing changes.
            let use_video_recorder = std::env::var("SCREENMIRROR_USE_VIDEO_RECORDER")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(true);
            if use_video_recorder {
                if let Some(monitor) = cached_monitor.as_ref() {
                    match monitor.video_recorder() {
                        Ok((recorder, receiver)) => match recorder.start() {
                            Ok(()) => {
                                tracing::info!("using xcap continuous screen recorder");
                                Some((recorder, receiver))
                            }
                            Err(error) => {
                                tracing::warn!("continuous screen recorder unavailable: {error}");
                                None
                            }
                        },
                        Err(error) => {
                            tracing::warn!("continuous screen recorder unavailable: {error}");
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                tracing::info!("using xcap capture_image polling for low-latency capture");
                None
            }
        };
        #[cfg(not(target_os = "macos"))]
        let mut screen_capture: Option<()> = None;
        let mut encoder_slot: Option<std::sync::Mutex<Option<VideoEncoder>>> = None;
        let mut frames: u32 = 0;
        while r.load(std::sync::atomic::Ordering::Relaxed) {
            let frame_index = frames;
            let capture_started = std::time::Instant::now();
            let captured = if frames == 0 {
                // Seed the encoder immediately. Some recorder implementations
                // do not emit an initial frame until a display change occurs.
                #[cfg(target_os = "macos")]
                {
                    capture_one_at_with_monitor(&target, frame_index, cached_monitor.as_ref())
                }
                #[cfg(not(target_os = "macos"))]
                {
                    capture_one_at(&target, frame_index)
                }
            } else if let Some((_, receiver)) = screen_capture.as_mut() {
                match receiver.recv_timeout(interval) {
                    Ok(mut raw_frame) => {
                        while let Ok(newer) = receiver.try_recv() {
                            raw_frame = newer;
                        }
                        RgbaImage::from_raw(raw_frame.width, raw_frame.height, raw_frame.raw)
                            .map(|rgba| captured_frame_from_rgba(rgba, target.quality))
                            .ok_or_else(|| "continuous capture returned invalid frame".to_string())
                    }
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => {
                        tracing::warn!("continuous screen recorder disconnected");
                        screen_capture = None;
                        continue;
                    }
                }
            } else {
                #[cfg(target_os = "macos")]
                {
                    capture_one_at_with_monitor(&target, frame_index, cached_monitor.as_ref())
                }
                #[cfg(not(target_os = "macos"))]
                {
                    capture_one_at(&target, frame_index)
                }
            };
            let capture_elapsed = capture_started.elapsed();
            if frames < 3 || capture_elapsed >= Duration::from_millis(100) {
                tracing::info!("video capture: capture/resize elapsed={:?}", capture_elapsed);
            }
            match captured {
                Ok(frame) => {
                    let dimensions = (frame.rgba.width(), frame.rgba.height());
                    let kbps = capture_bitrate_kbps(
                        dimensions.0,
                        dimensions.1,
                        fps,
                        target.quality,
                    );
                    if encoder_slot.is_none() {
                        encoder_slot = Some(std::sync::Mutex::new(None));
                    }
                    let encode_started = std::time::Instant::now();
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
                        let encoded = guard.as_mut().unwrap().encode(frame.rgba.as_raw());
                        encoded
                    };
                    let encode_elapsed = encode_started.elapsed();
                    if frames < 3 || encode_elapsed >= Duration::from_millis(100) {
                        tracing::info!(
                            "video capture: encode dimensions={}x{} elapsed={:?}",
                            dimensions.0,
                            dimensions.1,
                            encode_elapsed
                        );
                    }
                    match result {
                        Ok(encoded) => {
                            frames = frames.wrapping_add(1);
                            if frames <= 3 {
                                tracing::info!(
                                    "video capture: encoded frame #{} bytes={} keyframe={}",
                                    frames,
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

#[cfg(test)]
mod tests {
    use super::{capture_bitrate_kbps, normalize_captured_rgba_with_max_dim, normalize_encoder_dimensions, profile_max_dim};
    use image::RgbaImage;

    #[test]
    fn normalizes_odd_capture_dimensions_for_video_encoder() {
        assert_eq!(normalize_encoder_dimensions(1920, 1247), (1920, 1246));
        assert_eq!(normalize_encoder_dimensions(1919, 1247), (1918, 1246));
    }

    #[test]
    fn preserves_even_capture_dimensions() {
        assert_eq!(normalize_encoder_dimensions(1920, 1248), (1920, 1248));
    }

    #[test]
    fn high_resolution_capture_is_not_reduced_to_low_definition() {
        let frame = normalize_captured_rgba_with_max_dim(RgbaImage::new(1920, 1080), 1920);
        assert_eq!((frame.width(), frame.height()), (1920, 1080));
    }

    #[test]
    fn quality_profiles_raise_resolution_progressively() {
        assert_eq!(profile_max_dim(0.5), 1920);
        assert_eq!(profile_max_dim(0.75), 1920);
        assert_eq!(profile_max_dim(1.0), 3840);
    }

    #[test]
    fn screen_bitrate_scales_for_readable_text_without_lowering_fps() {
        assert!(capture_bitrate_kbps(1920, 1080, 15, 1.0) >= 3_500);
        assert!(capture_bitrate_kbps(1920, 1080, 15, 1.0) > capture_bitrate_kbps(960, 540, 15, 1.0));
    }
}

impl CaptureHandle {
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
