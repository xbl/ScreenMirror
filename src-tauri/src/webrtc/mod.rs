//! Screen capture -> H.264 -> RTP via str0m.

use image::RgbaImage;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub mod ffi;
pub mod host;
pub mod screencapturekit_capture;
pub mod video_toolbox;
pub mod video_toolbox_iosurface;
pub mod video_toolbox_native;

pub use host::HostPeer;
pub use screencapturekit_capture::{start_screen_capture, ScreenKitCapture, ScreenKitError, ScreenKitFrame};
pub use video_toolbox::{H264EncodedFrame, VideoEncoder};
pub use video_toolbox_iosurface::{
    is_available as iosurface_encoder_available, IOSurfaceEncoderError,
    IOSurfaceVideoEncoder,
};

#[derive(Debug, Clone, Copy)]
pub enum CaptureKind {
    Screen,
    Window,
    /// Synthetic moving gradient. Used by the E2E harness in environments
    /// without a real display so the host pipeline (encode + RTP + ICE + STAP-A)
    /// can be exercised end-to-end without xcap.
    TestPattern,
}

#[derive(Debug, Clone)]
pub struct CaptureTarget {
    pub kind: CaptureKind,
    /// Legacy source index, retained so existing callers keep working.
    pub id: u32,
    /// Stable native source identifier returned by `enumerate_sources`.
    pub source_id: Option<String>,
    /// 0.25 .. 1.0. Kept for compatibility with existing callers.
    pub quality: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSourceInfo {
    /// Legacy index-based identifier (for example, `screen:0`).
    pub id: String,
    /// Stable native identifier (for example, `69733440`).
    pub source_id: String,
    pub name: String,
    pub kind: String,
    pub is_primary: bool,
    /// Reserved for a future capture thumbnail data URL.
    pub preview: Option<String>,
    pub width: u32,
    pub height: u32,
}

fn legacy_capture_source_id(kind: &str, index: usize) -> String {
    format!("{kind}:{index}")
}

fn is_native_source_id(source_id: &str) -> bool {
    !source_id.is_empty()
        && !source_id.starts_with("screen:")
        && !source_id.starts_with("window:")
}

const SCREENKIT_MAX_CONSECUTIVE_TIMEOUTS: u8 = 3;

fn should_abandon_screenkit_after_timeouts(consecutive_timeouts: u8) -> bool {
    consecutive_timeouts >= SCREENKIT_MAX_CONSECUTIVE_TIMEOUTS
}

fn next_screenkit_timeout_count(consecutive_timeouts: u8, timed_out: bool) -> u8 {
    if timed_out {
        consecutive_timeouts.saturating_add(1)
    } else {
        0
    }
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
                id: legacy_capture_source_id("screen", idx),
                source_id: m.id().map(|id| id.to_string()).unwrap_or_default(),
                name: m.name().unwrap_or_else(|_| format!("Display {}", idx + 1)),
                kind: "screen".into(),
                is_primary: m.is_primary().unwrap_or(false),
                preview: None,
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
                    id: legacy_capture_source_id("window", idx),
                    source_id: w.id().map(|id| id.to_string()).unwrap_or_default(),
                    name,
                    kind: "window".into(),
                    is_primary: false,
                    preview: None,
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

#[cfg(target_os = "macos")]
fn select_source_by_id<T>(
    sources: Vec<T>,
    source_id: Option<&str>,
    fallback_index: u32,
    kind: &str,
    id: impl Fn(&T) -> Result<u32, xcap::XCapError>,
) -> Result<T, String> {
    if let Some(source_id) = source_id.filter(|id| is_native_source_id(id)) {
        return sources
            .into_iter()
            .find(|source| id(source).ok().is_some_and(|native_id| source_id == native_id.to_string()))
            .ok_or_else(|| format!("{kind} source {source_id} is no longer available"));
    }
    sources
        .into_iter()
        .nth(fallback_index as usize)
        .ok_or_else(|| format!("{kind} index {fallback_index} out of range"))
}

pub struct CapturedFrame {
    pub rgba: RgbaImage,
    pub captured_at: std::time::Instant,
    pub screenkit: Option<ScreenKitFrame>,
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
        // High uses 1920px to keep text readable while preserving interactive
        // motion. Ultra remains an explicit opt-in for devices that can
        // sustain larger frames.
        q if q >= 0.65 => 1920,
        _ => 1920,
    }
}

pub fn profile_fps(quality: f32) -> u32 {
    if quality >= 0.9 {
        20
    } else if quality >= 0.65 {
        30
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
        screenkit: None,
    }
}

/// Convert a ScreenCaptureKit BGRA readback into the RGBA format consumed by
/// the existing encoder. The native IOSurface encoder can bypass this copy
/// once it is available; until then this is the safe fallback path.
fn captured_frame_from_screenkit(
    frame: ScreenKitFrame,
    quality: f32,
) -> Result<CapturedFrame, String> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let stride = frame.bytes_per_row as usize;
    let native_frame = frame.clone();
    let bgra = frame
        .bgra
        .ok_or_else(|| "ScreenCaptureKit frame has no BGRA readback".to_string())?;
    if width == 0 || height == 0 || stride < width.saturating_mul(4) {
        return Err("ScreenCaptureKit frame has invalid dimensions".into());
    }
    if bgra.len() < stride.saturating_mul(height) {
        return Err("ScreenCaptureKit frame buffer is shorter than its stride".into());
    }
    let mut rgba = vec![0u8; width.saturating_mul(height).saturating_mul(4)];
    for y in 0..height {
        let src_row = y * stride;
        let dst_row = y * width * 4;
        for x in 0..width {
            let src = src_row + x * 4;
            let dst = dst_row + x * 4;
            rgba[dst] = bgra[src + 2];
            rgba[dst + 1] = bgra[src + 1];
            rgba[dst + 2] = bgra[src];
            rgba[dst + 3] = bgra[src + 3];
        }
    }
    let rgba = RgbaImage::from_raw(frame.width, frame.height, rgba)
        .ok_or_else(|| "ScreenCaptureKit RGBA dimensions do not match buffer".to_string())?;
    let mut captured = captured_frame_from_rgba(rgba, quality);
    captured.captured_at = frame.captured_at;
    captured.screenkit = Some(native_frame);
    Ok(captured)
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
                None => select_source_by_id(
                    Monitor::all().map_err(|e| e.to_string())?,
                    target.source_id.as_deref(),
                    target.id,
                    "screen",
                    |monitor| monitor.id(),
                )?,
            };
            monitor.capture_image()
        }
        CaptureKind::Window => select_source_by_id(
            Window::all().map_err(|e| e.to_string())?,
            target.source_id.as_deref(),
            target.id,
            "window",
            |window| window.id(),
        )?
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
    // Keep exactly one pending capture. A slow encoder must consume the newest
    // frame available, never a queue of already-stale frames.
    let frame_slot = Arc::new((Mutex::new(None::<CapturedFrame>), Condvar::new()));
    let encoder_frame_slot = frame_slot.clone();
    let encoder_running = running.clone();
    let encoder_sink = sink.clone();
    std::thread::spawn(move || {
        let mut encoder: Option<VideoEncoder> = None;
        let mut iosurface_encoder: Option<IOSurfaceVideoEncoder> = None;
        let mut iosurface_disabled = false;
        let mut encoded_count = 0u32;
        while encoder_running.load(std::sync::atomic::Ordering::Relaxed) {
            let mut frame = {
                let (lock, wake) = &*encoder_frame_slot;
                let mut slot = match lock.lock() {
                    Ok(slot) => slot,
                    Err(_) => break,
                };
                while slot.is_none() && encoder_running.load(std::sync::atomic::Ordering::Relaxed) {
                    match wake.wait_timeout(slot, Duration::from_millis(100)) {
                        Ok((next, _)) => slot = next,
                        Err(_) => return,
                    }
                }
                if !encoder_running.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                match slot.take() {
                    Some(frame) => frame,
                    None => continue,
                }
            };
            let dimensions = (frame.rgba.width(), frame.rgba.height());
            let encode_started = std::time::Instant::now();
            let native_result = if let Some(native_frame) = frame.screenkit.take() {
                if !iosurface_disabled {
                    if iosurface_encoder.is_none() {
                        let kbps = capture_bitrate_kbps(
                            native_frame.width,
                            native_frame.height,
                            fps,
                            target.quality,
                        );
                        match IOSurfaceVideoEncoder::new(
                            native_frame.width & !1,
                            native_frame.height & !1,
                            fps.max(1),
                            kbps,
                        ) {
                            Ok(value) => iosurface_encoder = Some(value),
                            Err(error) => {
                                tracing::warn!("native IOSurface encoder unavailable; falling back to FFmpeg: {error}");
                                iosurface_disabled = true;
                            }
                        }
                    }
                    if let Some(native_encoder) = iosurface_encoder.as_mut() {
                        match native_encoder.encode(native_frame) {
                            Ok(encoded) => Some(Ok(encoded)),
                            Err(error) => {
                                tracing::warn!("native IOSurface encode failed; falling back to FFmpeg: {error}");
                                iosurface_encoder = None;
                                iosurface_disabled = true;
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            let result = if let Some(result) = native_result {
                result
            } else {
                if encoder.is_none() {
                let kbps = capture_bitrate_kbps(dimensions.0, dimensions.1, fps, target.quality);
                match VideoEncoder::new(dimensions.0, dimensions.1, fps.max(1), kbps) {
                    Ok(value) => encoder = Some(value),
                    Err(error) => {
                        tracing::warn!("video encoder initialization failed: {error}");
                        continue;
                    }
                }
                }
                encoder.as_mut().unwrap().encode(frame.rgba.as_raw())
            };
            let elapsed = encode_started.elapsed();
            if encoded_count < 3 || elapsed >= Duration::from_millis(100) {
                tracing::info!("video encode dimensions={}x{} elapsed={:?}", dimensions.0, dimensions.1, elapsed);
            }
            match result {
                Ok(mut encoded) => {
                    encoded_count = encoded_count.wrapping_add(1);
                    encoded.captured_at = frame.captured_at;
                    // Native VideoToolbox may spend 300-400ms producing a frame on
                    // high-resolution displays. Keep that bounded-latency frame
                    // rather than starving the viewer after its first keyframe.
                    if encoded.captured_at.elapsed() <= Duration::from_millis(500) || encoded.keyframe {
                        encoder_sink(encoded);
                    }
                }
                Err(error) => tracing::warn!("H.264 encode error: {error}"),
            }
        }
    });
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
                .and_then(|monitors| select_source_by_id(
                    monitors,
                    target.source_id.as_deref(),
                    target.id,
                    "screen",
                    |monitor| monitor.id(),
                ))
                .ok()
        } else {
            None
        };
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        let use_screenkit_capture = matches!(target.kind, CaptureKind::Screen)
            && std::env::var("SCREENMIRROR_USE_IOSURFACE")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
        #[cfg(all(target_os = "macos", not(feature = "screenkit")))]
        let use_screenkit_capture = false;
        #[cfg(target_os = "macos")]
        let mut screen_capture = {
            // On some macOS versions video_recorder() is change-driven and
            // does not emit an initial frame. The loop seeds one frame with
            // capture_image() before using the recorder for ongoing changes.
            let use_video_recorder = std::env::var("SCREENMIRROR_USE_VIDEO_RECORDER")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                // Recorder mode provides materially steadier frame pacing on
                // high-resolution displays. Direct polling remains available
                // explicitly for diagnostics and very low-latency setups.
                .unwrap_or(true);
            if use_video_recorder && !use_screenkit_capture {
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
        #[cfg(target_os = "macos")]
        let mut screenkit_capture = if use_screenkit_capture {
            match start_screen_capture(target.clone(), fps) {
                Ok(capture) => {
                    tracing::info!(
                        "using ScreenCaptureKit latest-frame capture (IOSurface encoder available={})",
                        iosurface_encoder_available()
                    );
                    Some(capture)
                }
                Err(error) => {
                    tracing::warn!("ScreenCaptureKit unavailable; falling back to xcap: {error}");
                    None
                }
            }
        } else {
            None
        };
        #[cfg(not(target_os = "macos"))]
        let mut screen_capture: Option<()> = None;
        #[cfg(not(target_os = "macos"))]
        let mut screenkit_capture: Option<ScreenKitCapture> = None;
        #[cfg(all(target_os = "macos", not(feature = "screenkit")))]
        let mut screenkit_capture: Option<ScreenKitCapture> = None;
        let mut screenkit_consecutive_timeouts = 0_u8;
        let mut frames: u32 = 0;
        while r.load(std::sync::atomic::Ordering::Relaxed) {
            let frame_index = frames;
            let capture_started = std::time::Instant::now();
            let screenkit_result = screenkit_capture
                .as_ref()
                .map(|capture| capture.recv_timeout(interval));
            let captured = if let Some(screenkit_result) = screenkit_result {
                match screenkit_result {
                    Ok(frame) => {
                        screenkit_consecutive_timeouts =
                            next_screenkit_timeout_count(screenkit_consecutive_timeouts, false);
                        captured_frame_from_screenkit(frame, target.quality)
                    }
                    Err(ScreenKitError::Unavailable(_)) => {
                        screenkit_consecutive_timeouts =
                            next_screenkit_timeout_count(screenkit_consecutive_timeouts, true);
                        if should_abandon_screenkit_after_timeouts(
                            screenkit_consecutive_timeouts,
                        ) {
                            tracing::warn!(
                                "ScreenCaptureKit timed out {} consecutive intervals; falling back to xcap",
                                screenkit_consecutive_timeouts
                            );
                            if let Some(capture) = screenkit_capture.take() {
                                capture.stop();
                            }
                        }
                        continue;
                    }
                    Err(ScreenKitError::Stopped) => {
                        tracing::warn!("ScreenCaptureKit stopped; falling back to xcap");
                        screenkit_capture = None;
                        continue;
                    }
                    Err(ScreenKitError::UnsupportedPlatform) => {
                        screenkit_capture = None;
                        continue;
                    }
                }
            } else if frames == 0 {
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
                    frames = frames.wrapping_add(1);
                    let (slot, wake) = &*frame_slot;
                    match slot.lock() {
                        Ok(mut current) => {
                            *current = Some(frame);
                            wake.notify_one();
                        }
                        Err(_) => break,
                    }
                }
                Err(error) => tracing::warn!("capture error: {error}"),
            }
            // `video_recorder()` already blocks in `recv_timeout(interval)`.
            // Sleeping again here halves the effective frame rate and adds up
            // to one full frame of avoidable latency. Polling capture still
            // needs the pacing sleep.
            if screen_capture.is_none() && screenkit_capture.is_none() {
                std::thread::sleep(interval);
            }
        }
    });
    CaptureHandle { running }
}

pub struct CaptureHandle {
    running: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
mod tests {
    use super::{
        capture_bitrate_kbps, legacy_capture_source_id, normalize_captured_rgba_with_max_dim,
        normalize_encoder_dimensions, next_screenkit_timeout_count, profile_max_dim,
        should_abandon_screenkit_after_timeouts,
    };
    #[cfg(target_os = "macos")]
    use super::select_source_by_id;
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

    #[test]
    fn legacy_screen_source_id_uses_the_enumeration_index() {
        assert_eq!(legacy_capture_source_id("screen", 2), "screen:2");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn source_id_selects_the_matching_native_source_before_the_legacy_index() {
        let source = select_source_by_id(
            vec![41_u32, 99_u32],
            Some("99"),
            0,
            "screen",
            |id| Ok(*id),
        )
        .expect("stable source id selects a source");

        assert_eq!(source, 99);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn missing_source_id_falls_back_to_the_legacy_index() {
        let source = select_source_by_id(
            vec![41_u32, 99_u32],
            None,
            1,
            "screen",
            |id| Ok(*id),
        )
        .expect("legacy index selects a source");

        assert_eq!(source, 99);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn empty_source_id_falls_back_to_the_legacy_index() {
        let source = select_source_by_id(
            vec![41_u32, 99_u32],
            Some(""),
            1,
            "screen",
            |id| Ok(*id),
        )
        .expect("empty source id falls back to its index");

        assert_eq!(source, 99);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn legacy_screen_source_id_falls_back_to_the_legacy_index() {
        let source = select_source_by_id(
            vec![41_u32, 99_u32],
            Some("screen:1"),
            1,
            "screen",
            |id| Ok(*id),
        )
        .expect("legacy source id falls back to its index");

        assert_eq!(source, 99);
    }

    #[test]
    fn screenkit_timeout_threshold_allows_three_missed_intervals() {
        assert!(!should_abandon_screenkit_after_timeouts(2));
        assert!(should_abandon_screenkit_after_timeouts(3));
        assert_eq!(next_screenkit_timeout_count(2, false), 0);
    }
}

impl CaptureHandle {
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
