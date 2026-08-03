//! Screen capture -> H.264 -> RTP via str0m.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::codecs::jpeg::JpegEncoder;
use image::RgbaImage;
#[cfg(any(target_os = "macos", test))]
use std::collections::{HashMap, HashSet};
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
pub use screencapturekit_capture::{
    start_screen_capture, ScreenKitCapture, ScreenKitError, ScreenKitFrame,
};
pub use video_toolbox::{H264EncodedFrame, VideoEncoder};
pub use video_toolbox_iosurface::{
    is_available as iosurface_encoder_available, IOSurfaceEncoderError, IOSurfaceVideoEncoder,
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
    /// A compact JPEG thumbnail data URL when one could be captured.
    pub preview: Option<String>,
    pub width: u32,
    pub height: u32,
}

const PREVIEW_MAX_DIMENSION: u32 = 320;
const PREVIEW_JPEG_QUALITY: u8 = 60;

#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct PreviewCacheKey {
    source_kind: String,
    source_id: String,
    width: u32,
    height: u32,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Default)]
struct DisplayPreviewCache {
    previews: HashMap<PreviewCacheKey, String>,
    epochs: HashMap<PreviewCacheKey, u64>,
    in_flight: HashSet<PreviewCacheKey>,
}

#[cfg(target_os = "macos")]
static DISPLAY_PREVIEW_CACHE: std::sync::OnceLock<Mutex<DisplayPreviewCache>> =
    std::sync::OnceLock::new();

#[cfg(any(target_os = "macos", test))]
fn begin_preview_request(
    cache: &mut DisplayPreviewCache,
    key: &PreviewCacheKey,
    force_refresh: bool,
) -> (u64, Option<String>, bool) {
    let epoch = *cache.epochs.get(key).unwrap_or(&0);
    if !force_refresh {
        if let Some(preview) = cache.previews.get(key).cloned() {
            return (epoch, Some(preview), false);
        }
    }
    if !cache.in_flight.insert(key.clone()) {
        return (epoch, None, false);
    }

    if force_refresh {
        let epoch = cache.epochs.entry(key.clone()).or_insert(0);
        *epoch = epoch.wrapping_add(1);
        cache.previews.remove(key);
        return (*epoch, None, true);
    }

    (epoch, None, true)
}

#[cfg(any(target_os = "macos", test))]
fn store_preview_if_current(
    cache: &mut DisplayPreviewCache,
    key: &PreviewCacheKey,
    epoch: u64,
    preview: String,
) {
    if cache.epochs.get(key).copied().unwrap_or(0) == epoch {
        cache.previews.insert(key.clone(), preview);
    }
}

#[cfg(any(target_os = "macos", test))]
fn finish_preview_request(
    cache: &mut DisplayPreviewCache,
    key: &PreviewCacheKey,
    epoch: u64,
    preview: Option<String>,
) {
    cache.in_flight.remove(key);
    if let Some(preview) = preview {
        store_preview_if_current(cache, key, epoch, preview);
    }
}

fn preview_from_capture_result(capture: Result<RgbaImage, String>) -> Option<String> {
    capture.ok().and_then(|rgba| preview_data_url(rgba).ok())
}

fn preview_dimensions(width: u32, height: u32) -> (u32, u32) {
    let longest_edge = width.max(height);
    if longest_edge <= PREVIEW_MAX_DIMENSION {
        return (width, height);
    }

    let scale = PREVIEW_MAX_DIMENSION as f64 / longest_edge as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

fn preview_data_url(rgba: RgbaImage) -> Result<String, String> {
    let (width, height) = preview_dimensions(rgba.width(), rgba.height());
    let thumbnail =
        image::imageops::resize(&rgba, width, height, image::imageops::FilterType::Triangle);
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, PREVIEW_JPEG_QUALITY)
        .encode_image(&thumbnail)
        .map_err(|error| format!("failed to encode display preview: {error}"))?;

    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes)))
}

#[cfg(target_os = "macos")]
fn capture_source_preview(
    source_id: &str,
    source_kind: &str,
    width: u32,
    height: u32,
    force_refresh: bool,
    capture: impl FnOnce() -> Result<RgbaImage, String>,
) -> Option<String> {
    let key = PreviewCacheKey {
        source_kind: source_kind.into(),
        source_id: source_id.into(),
        width,
        height,
    };
    let cache = DISPLAY_PREVIEW_CACHE.get_or_init(|| Mutex::new(DisplayPreviewCache::default()));
    let (epoch, cached_preview, should_capture) = match cache.lock() {
        Ok(mut cache) => begin_preview_request(&mut cache, &key, force_refresh),
        Err(_) => (0, None, false),
    };
    if let Some(preview) = cached_preview {
        return Some(preview);
    }
    if !should_capture {
        return None;
    }

    let capture = capture();
    if capture.is_err() {
        tracing::debug!(source_id, source_kind, "could not capture source preview");
    }
    let preview = preview_from_capture_result(capture);

    if let Ok(mut cache) = cache.lock() {
        finish_preview_request(&mut cache, &key, epoch, preview.clone());
    }
    preview
}

fn legacy_capture_source_id(kind: &str, index: usize) -> String {
    format!("{kind}:{index}")
}

fn is_shareable_window(app_name: &str, title: &str, width: u32, height: u32) -> bool {
    const SYSTEM_APPS: [&str; 5] = [
        "Window Server",
        "SystemUIServer",
        "Control Center",
        "Notification Center",
        "Dock",
    ];
    const SYSTEM_WINDOWS: [&str; 3] = ["Menu Bar", "StatusIndicator", "Desktop"];

    let app_name = app_name.trim();
    let title = title.trim();
    !app_name.is_empty()
        && width >= 160
        && height >= 80
        && !SYSTEM_APPS
            .iter()
            .any(|system_app| app_name.eq_ignore_ascii_case(system_app))
        && !SYSTEM_WINDOWS
            .iter()
            .any(|system_window| title.eq_ignore_ascii_case(system_window))
}

fn is_native_source_id(source_id: &str) -> bool {
    !source_id.is_empty() && !source_id.starts_with("screen:") && !source_id.starts_with("window:")
}

/// Resolve a source against the stable native identifiers emitted during
/// enumeration. Only absent, empty, or legacy `screen:N` / `window:N` values
/// may use the legacy index; a missing native identifier must not silently
/// select a different display after a topology change.
fn select_source_index(
    source_ids: &[String],
    source_id: Option<&str>,
    fallback_index: u32,
    kind: &str,
) -> Result<usize, String> {
    if let Some(source_id) = source_id.filter(|id| is_native_source_id(id)) {
        return source_ids
            .iter()
            .position(|native_id| native_id == source_id)
            .ok_or_else(|| format!("{kind} source {source_id} is no longer available"));
    }

    let index = usize::try_from(fallback_index)
        .map_err(|_| format!("{kind} index {fallback_index} out of range"))?;
    if index < source_ids.len() {
        Ok(index)
    } else {
        Err(format!("{kind} index {fallback_index} out of range"))
    }
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
            let source_id = m.id().map(|id| id.to_string()).map_err(|error| {
                format!("failed to read display ID for screen {}: {error}", idx + 1)
            })?;
            let width = m.width().unwrap_or(0);
            let height = m.height().unwrap_or(0);
            out.push(CaptureSourceInfo {
                id: legacy_capture_source_id("screen", idx),
                preview: None,
                source_id,
                name: m.name().unwrap_or_else(|_| format!("Display {}", idx + 1)),
                kind: "screen".into(),
                is_primary: m.is_primary().unwrap_or(false),
                width,
                height,
            });
        }
        if let Ok(windows) = Window::all() {
            let mut window_index = 0;
            for (idx, w) in windows.into_iter().enumerate() {
                let app_name = w.app_name().unwrap_or_default();
                let title = w.title().unwrap_or_default();
                let width = w.width().unwrap_or(0);
                let height = w.height().unwrap_or(0);
                if !is_shareable_window(&app_name, &title, width, height) {
                    continue;
                }
                let name = if title.is_empty() { app_name } else { title };
                out.push(CaptureSourceInfo {
                    id: legacy_capture_source_id("window", window_index),
                    source_id: w.id().map(|id| id.to_string()).map_err(|error| {
                        format!("failed to read native ID for window {idx}: {error}")
                    })?,
                    name,
                    kind: "window".into(),
                    is_primary: false,
                    preview: None,
                    width,
                    height,
                });
                window_index += 1;
            }
        }
        Ok(out)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}

/// Capture a compact preview for one display after the source metadata has
/// already been returned to the UI. This intentionally does not participate in
/// the video capture loop.
pub fn get_capture_source_preview(
    source_id: &str,
    force_refresh: bool,
    source_kind: &str,
) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        use xcap::{Monitor, Window};

        match source_kind {
            "window" => {
                let window = Window::all()
                    .map_err(|error| error.to_string())?
                    .into_iter()
                    .find(|window| window.id().map(|id| id.to_string()).ok().as_deref() == Some(source_id));
                let Some(window) = window else { return Ok(None); };
                let width = window.width().unwrap_or(0);
                let height = window.height().unwrap_or(0);
                Ok(capture_source_preview(source_id, source_kind, width, height, force_refresh, || {
                    window
                        .capture_image()
                        .map_err(|error| error.to_string())
                        .and_then(|image| RgbaImage::from_raw(image.width(), image.height(), image.into_raw()).ok_or_else(|| "window preview has invalid image dimensions".into()))
                }))
            }
            _ => {
                let monitor = Monitor::all()
                    .map_err(|error| error.to_string())?
                    .into_iter()
                    .find(|monitor| monitor.id().map(|id| id.to_string()).ok().as_deref() == Some(source_id));
                let Some(monitor) = monitor else { return Ok(None); };
                let width = monitor.width().unwrap_or(0);
                let height = monitor.height().unwrap_or(0);
                Ok(capture_source_preview(source_id, source_kind, width, height, force_refresh, || {
                    monitor
                        .capture_image()
                        .map_err(|error| error.to_string())
                        .and_then(|image| RgbaImage::from_raw(image.width(), image.height(), image.into_raw()).ok_or_else(|| "display preview has invalid image dimensions".into()))
                }))
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (source_id, force_refresh);
        Ok(None)
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
    let source_ids = sources
        .iter()
        .map(|source| {
            id(source)
                .map(|native_id| native_id.to_string())
                .map_err(|error| format!("failed to read native ID for {kind}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let index = select_source_index(&source_ids, source_id, fallback_index, kind)?;
    sources
        .into_iter()
        .nth(index)
        .ok_or_else(|| format!("{kind} index {fallback_index} out of range"))
}

pub struct CapturedFrame {
    /// CPU RGBA is optional for ScreenCaptureKit frames. Native IOSurface
    /// encoding consumes the original BGRA payload directly; only the
    /// software fallback needs the converted image.
    pub rgba: Option<RgbaImage>,
    pub captured_at: std::time::Instant,
    pub screenkit: Option<ScreenKitFrame>,
}

fn normalize_encoder_dimensions(width: u32, height: u32) -> (u32, u32) {
    (width & !1, height & !1)
}

/// Choose an even capture size while preserving the source aspect ratio.
/// ScreenCaptureKit performs this scale on the capture path, before the
/// per-frame BGRA copy reaches the CPU.
#[cfg(any(test, all(target_os = "macos", feature = "screenkit")))]
fn capture_dimensions(width: u32, height: u32, max_dim: u32) -> (u32, u32) {
    let max_dim = max_dim.max(2);
    if width <= max_dim && height <= max_dim {
        return normalize_encoder_dimensions(width.max(2), height.max(2));
    }
    let scale = max_dim as f64 / width.max(height) as f64;
    let scaled_width = ((width as f64 * scale).round() as u32).max(2);
    let scaled_height = ((height as f64 * scale).round() as u32).max(2);
    normalize_encoder_dimensions(scaled_width, scaled_height)
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
            dst[dst_offset..dst_offset + 4].copy_from_slice(&src[src_offset..src_offset + 4]);
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
        _ => 1280,
    }
}

pub fn profile_fps(quality: f32) -> u32 {
    if quality >= 0.9 {
        15
    } else if quality >= 0.65 {
        15
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
    let kbps =
        ((pixels as f64 * fps.max(1) as f64 * 12.0 * quality as f64) / 100_000.0).round() as u64;
    kbps.clamp(1_200, 20_000) as u32
}

fn captured_frame_from_rgba(rgba: RgbaImage, quality: f32) -> CapturedFrame {
    CapturedFrame {
        rgba: Some(normalize_captured_rgba(rgba, quality)),
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
    if width == 0 || height == 0 || stride < width.saturating_mul(4) {
        return Err("ScreenCaptureKit frame has invalid dimensions".into());
    }
    if frame
        .bgra
        .as_ref()
        .map_or(true, |bgra| bgra.len() < stride.saturating_mul(height))
    {
        return Err("ScreenCaptureKit frame buffer is shorter than its stride".into());
    }
    let _ = quality;
    Ok(CapturedFrame {
        rgba: None,
        captured_at: frame.captured_at,
        screenkit: Some(frame),
    })
}

fn screenkit_frame_to_rgba(frame: &ScreenKitFrame, quality: f32) -> Result<RgbaImage, String> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let stride = frame.bytes_per_row as usize;
    let bgra = frame
        .bgra
        .as_ref()
        .ok_or_else(|| "ScreenCaptureKit frame has no BGRA readback".to_string())?;
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
    Ok(normalize_captured_rgba(rgba, quality))
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
            let dimensions = frame
                .rgba
                .as_ref()
                .map(|rgba| (rgba.width(), rgba.height()))
                .or_else(|| {
                    frame
                        .screenkit
                        .as_ref()
                        .map(|native| normalize_encoder_dimensions(native.width, native.height))
                })
                .unwrap_or((0, 0));
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
                                frame.rgba =
                                    screenkit_frame_to_rgba(&native_frame, target.quality).ok();
                                iosurface_disabled = true;
                            }
                        }
                    }
                    if let Some(native_encoder) = iosurface_encoder.as_mut() {
                        match native_encoder.encode(&native_frame) {
                            Ok(encoded) => Some(Ok(encoded)),
                            Err(error) => {
                                tracing::warn!("native IOSurface encode failed; falling back to FFmpeg: {error}");
                                frame.rgba =
                                    screenkit_frame_to_rgba(&native_frame, target.quality).ok();
                                iosurface_encoder = None;
                                iosurface_disabled = true;
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    frame.rgba = screenkit_frame_to_rgba(&native_frame, target.quality).ok();
                    None
                }
            } else {
                None
            };
            let result = if let Some(result) = native_result {
                result
            } else {
                let rgba = match frame.rgba.take() {
                    Some(rgba) => rgba,
                    None => match frame.screenkit.as_ref() {
                        Some(native) => match screenkit_frame_to_rgba(native, target.quality) {
                            Ok(rgba) => rgba,
                            Err(error) => {
                                tracing::warn!(
                                    "ScreenCaptureKit fallback conversion failed: {error}"
                                );
                                continue;
                            }
                        },
                        None => {
                            tracing::warn!("captured frame has neither RGBA nor native pixels");
                            continue;
                        }
                    },
                };
                let rgba_dimensions = (rgba.width(), rgba.height());
                if encoder
                    .as_ref()
                    .map(|value| value.dimensions() != rgba_dimensions)
                    .unwrap_or(true)
                {
                    let kbps =
                        capture_bitrate_kbps(rgba_dimensions.0, rgba_dimensions.1, fps, target.quality);
                    match VideoEncoder::new(
                        rgba_dimensions.0,
                        rgba_dimensions.1,
                        fps.max(1),
                        kbps,
                    ) {
                        Ok(value) => encoder = Some(value),
                        Err(error) => {
                            tracing::warn!("video encoder initialization failed: {error}");
                            continue;
                        }
                    }
                }
                encoder.as_mut().unwrap().encode(rgba.as_raw())
            };
            let elapsed = encode_started.elapsed();
            if encoded_count < 3 || elapsed >= Duration::from_millis(100) {
                tracing::info!(
                    "video encode dimensions={}x{} elapsed={:?}",
                    dimensions.0,
                    dimensions.1,
                    elapsed
                );
            }
            match result {
                Ok(mut encoded) => {
                    encoded_count = encoded_count.wrapping_add(1);
                    encoded.captured_at = frame.captured_at;
                    // Native VideoToolbox may spend 300-400ms producing a frame on
                    // high-resolution displays. Keep that bounded-latency frame
                    // rather than starving the viewer after its first keyframe.
                    if encoded.captured_at.elapsed() <= Duration::from_millis(500)
                        || encoded.keyframe
                    {
                        encoder_sink(encoded);
                    }
                }
                Err(error) if error.starts_with("expected ") => {
                    tracing::error!("H.264 encode invariant violation: {error}")
                }
                // VideoToolbox can legitimately have no packet during its
                // startup delay. This is not an encode failure; the next
                // captured frame will drain the delayed packet.
                Err(error) if error == "encoder buffering" => {}
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
                .and_then(|monitors| {
                    select_source_by_id(
                        monitors,
                        target.source_id.as_deref(),
                        target.id,
                        "screen",
                        |monitor| monitor.id(),
                    )
                })
                .ok()
        } else {
            None
        };
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
        let use_screenkit_capture = matches!(target.kind, CaptureKind::Screen | CaptureKind::Window)
            && std::env::var("SCREENMIRROR_USE_IOSURFACE")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                // ScreenCaptureKit delivers frames at the requested cadence;
                // xcap's recorder can block for 200ms+ on high-DPI displays.
                // Keep an explicit opt-out for diagnostics and older macOS.
                .unwrap_or(true);
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
        #[cfg(all(target_os = "macos", feature = "screenkit"))]
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
        let mut unavailable_source_errors = 0_u8;
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
                        if should_abandon_screenkit_after_timeouts(screenkit_consecutive_timeouts) {
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
                tracing::info!(
                    "video capture: capture/resize elapsed={:?}",
                    capture_elapsed
                );
            }
            match captured {
                Ok(frame) => {
                    unavailable_source_errors = 0;
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
                Err(error) if error.contains("source") && error.contains("no longer available") => {
                    unavailable_source_errors = unavailable_source_errors.saturating_add(1);
                    if unavailable_source_errors == 1 {
                        tracing::warn!("capture stopped: {error}");
                    }
                    // A closed window cannot become valid again by polling the
                    // same native ID. Stop this producer so the host can
                    // release it and the user can choose a new source.
                    if unavailable_source_errors >= 3 {
                        break;
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
        begin_preview_request, capture_bitrate_kbps, capture_dimensions, finish_preview_request,
        is_shareable_window, legacy_capture_source_id, next_screenkit_timeout_count,
        normalize_captured_rgba_with_max_dim, normalize_encoder_dimensions, preview_dimensions,
        preview_from_capture_result, profile_fps, profile_max_dim, select_source_index,
        should_abandon_screenkit_after_timeouts, store_preview_if_current, DisplayPreviewCache,
        PreviewCacheKey,
    };
    use base64::{engine::general_purpose::STANDARD, Engine as _};
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
        assert_eq!(profile_max_dim(0.5), 1280);
        assert_eq!(profile_max_dim(0.75), 1920);
        assert_eq!(profile_max_dim(1.0), 3840);
    }

    #[test]
    fn capture_dimensions_scale_retina_inputs_once() {
        assert_eq!(capture_dimensions(3024, 1964, 1920), (1920, 1246));
        assert_eq!(capture_dimensions(1920, 1080, 1920), (1920, 1080));
        assert_eq!(capture_dimensions(1511, 981, 1920), (1510, 980));
    }

    #[test]
    fn high_and_ultra_profiles_prioritize_readable_frames_at_fifteen_fps() {
        assert_eq!(profile_fps(0.5), 30);
        assert_eq!(profile_fps(0.75), 15);
        assert_eq!(profile_fps(1.0), 15);
    }

    #[test]
    fn screen_bitrate_scales_for_readable_text_without_lowering_fps() {
        assert!(capture_bitrate_kbps(1920, 1080, 15, 1.0) >= 3_500);
        assert!(
            capture_bitrate_kbps(1920, 1080, 15, 1.0) > capture_bitrate_kbps(960, 540, 15, 1.0)
        );
    }

    #[test]
    fn legacy_screen_source_id_uses_the_enumeration_index() {
        assert_eq!(legacy_capture_source_id("screen", 2), "screen:2");
    }

    #[test]
    fn filters_macos_system_windows_from_shareable_sources() {
        assert!(!is_shareable_window("SystemUIServer", "Menu Bar", 1440, 24));
        assert!(!is_shareable_window("Window Server", "", 1440, 900));
        assert!(!is_shareable_window("Terminal", "StatusIndicator", 800, 600));
        assert!(!is_shareable_window("Terminal", "Small palette", 120, 60));
    }

    #[test]
    fn keeps_real_app_windows_in_shareable_sources() {
        assert!(is_shareable_window("Terminal", "bash", 900, 600));
        assert!(is_shareable_window("Terminal", "", 900, 600));
    }

    #[test]
    fn source_id_selects_the_matching_native_source_before_the_legacy_index() {
        let index = select_source_index(&["41".into(), "99".into()], Some("99"), 0, "screen")
            .expect("stable source id selects a source");

        assert_eq!(index, 1);
    }

    #[test]
    fn missing_source_id_falls_back_to_the_legacy_index() {
        let index = select_source_index(&["41".into(), "99".into()], None, 1, "screen")
            .expect("legacy index selects a source");

        assert_eq!(index, 1);
    }

    #[test]
    fn empty_source_id_falls_back_to_the_legacy_index() {
        let index = select_source_index(&["41".into(), "99".into()], Some(""), 1, "screen")
            .expect("empty source id falls back to its index");

        assert_eq!(index, 1);
    }

    #[test]
    fn legacy_screen_source_id_falls_back_to_the_legacy_index() {
        let index = select_source_index(&["41".into(), "99".into()], Some("screen:1"), 1, "screen")
            .expect("legacy source id falls back to its index");

        assert_eq!(index, 1);
    }

    #[test]
    fn legacy_window_source_id_falls_back_to_the_legacy_index() {
        let index = select_source_index(&["41".into(), "99".into()], Some("window:0"), 1, "window")
            .expect("legacy source id falls back to its index");

        assert_eq!(index, 1);
    }

    #[test]
    fn missing_native_source_id_is_an_explicit_error() {
        let error = select_source_index(&["41".into()], Some("99"), 0, "screen")
            .expect_err("a missing stable id must not select the fallback index");

        assert_eq!(error, "screen source 99 is no longer available");
    }

    #[test]
    fn out_of_range_legacy_index_is_an_explicit_error() {
        let error = select_source_index(&["41".into()], None, 1, "screen")
            .expect_err("out of range index must fail");

        assert_eq!(error, "screen index 1 out of range");
    }

    #[test]
    fn screenkit_timeout_threshold_allows_three_missed_intervals() {
        assert!(!should_abandon_screenkit_after_timeouts(2));
        assert!(should_abandon_screenkit_after_timeouts(3));
        assert_eq!(next_screenkit_timeout_count(2, false), 0);
    }

    #[test]
    fn preview_jpeg_is_bounded_to_320_pixels() {
        let preview = preview_from_capture_result(Ok(RgbaImage::new(2560, 1600)))
            .expect("a captured display produces a preview");
        let encoded = preview
            .strip_prefix("data:image/jpeg;base64,")
            .expect("preview is a JPEG data URL");
        let bytes = STANDARD.decode(encoded).expect("preview base64 decodes");
        let image = image::load_from_memory(&bytes).expect("preview JPEG decodes");

        assert!(image.width().max(image.height()) <= 320);
        assert_eq!((image.width(), image.height()), (320, 200));
    }

    #[test]
    fn preview_dimensions_leave_small_images_unchanged() {
        assert_eq!(preview_dimensions(160, 90), (160, 90));
    }

    #[test]
    fn failed_preview_capture_falls_back_to_none() {
        let preview = preview_from_capture_result(Err("Screen Recording permission denied".into()));

        assert!(preview.is_none());
    }

    #[test]
    fn forced_preview_refresh_evicts_old_cache_after_capture_failure() {
        let key = PreviewCacheKey {
            source_kind: "screen".into(),
            source_id: "display-1".into(),
            width: 1920,
            height: 1080,
        };
        let mut cache = DisplayPreviewCache::default();
        cache.previews.insert(key.clone(), "old-preview".into());

        let (old_epoch, cached, _) = begin_preview_request(&mut cache, &key, false);
        assert_eq!(cached.as_deref(), Some("old-preview"));

        let (forced_epoch, forced_cached, forced_started) =
            begin_preview_request(&mut cache, &key, true);
        assert!(forced_cached.is_none());
        assert!(forced_started);
        // Simulate an older capture completing after the forced one failed.
        store_preview_if_current(&mut cache, &key, old_epoch, "stale-preview".into());
        finish_preview_request(&mut cache, &key, forced_epoch, None);

        let (_, cached_after_failure, should_capture_after_failure) =
            begin_preview_request(&mut cache, &key, false);
        assert!(cached_after_failure.is_none());
        assert!(should_capture_after_failure);
    }

    #[test]
    fn preview_captures_are_single_flight_per_source() {
        let key = PreviewCacheKey {
            source_kind: "screen".into(),
            source_id: "display-1".into(),
            width: 1920,
            height: 1080,
        };
        let mut cache = DisplayPreviewCache::default();

        let (_, _, started) = begin_preview_request(&mut cache, &key, false);
        let (_, cached, force_started) = begin_preview_request(&mut cache, &key, true);

        assert!(started);
        assert!(cached.is_none());
        assert!(!force_started);
    }
}

impl CaptureHandle {
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
