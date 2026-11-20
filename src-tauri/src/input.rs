//! Remote touch and mouse control input injection.
//!
//! Handles coordinate translation from normalized browser space (0.0..1.0)
//! to macOS global desktop coordinates, and posts synthetic CoreGraphics events.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl Default for MouseButton {
    fn default() -> Self {
        Self::Left
    }
}

fn default_click_count() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RemoteInputEvent {
    MouseMove {
        x: f64,
        y: f64,
    },
    MouseDown {
        x: f64,
        y: f64,
        #[serde(default)]
        button: MouseButton,
        #[serde(default = "default_click_count")]
        click_count: u32,
    },
    MouseUp {
        x: f64,
        y: f64,
        #[serde(default)]
        button: MouseButton,
        #[serde(default = "default_click_count")]
        click_count: u32,
    },
    MouseClick {
        x: f64,
        y: f64,
        #[serde(default)]
        button: MouseButton,
        #[serde(default = "default_click_count")]
        click_count: u32,
    },
    MouseDrag {
        x: f64,
        y: f64,
        #[serde(default)]
        button: MouseButton,
    },
    Scroll {
        x: f64,
        y: f64,
        #[serde(default)]
        delta_x: f64,
        #[serde(default)]
        delta_y: f64,
    },
}

impl RemoteInputEvent {
    pub fn normalized_coords(&self) -> (f64, f64) {
        match self {
            Self::MouseMove { x, y } => (*x, *y),
            Self::MouseDown { x, y, .. } => (*x, *y),
            Self::MouseUp { x, y, .. } => (*x, *y),
            Self::MouseClick { x, y, .. } => (*x, *y),
            Self::MouseDrag { x, y, .. } => (*x, *y),
            Self::Scroll { x, y, .. } => (*x, *y),
        }
    }
}

/// Parses remote input JSON text from either direct format or wrapped format.
pub fn parse_remote_input(text: &str) -> Result<RemoteInputEvent, String> {
    if let Ok(event) = serde_json::from_str::<RemoteInputEvent>(text) {
        return Ok(event);
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(payload) = value.get("payload") {
            if let Ok(event) = serde_json::from_value::<RemoteInputEvent>(payload.clone()) {
                return Ok(event);
            }
        }

        // Support { "type": "mouse_move", ... } where type was used instead of action
        if let Some(msg_type) = value.get("type").and_then(|v| v.as_str()) {
            let mut val_clone = value.clone();
            if let Some(map) = val_clone.as_object_mut() {
                map.insert("action".into(), serde_json::Value::String(msg_type.to_lowercase()));
                if let Ok(event) = serde_json::from_value::<RemoteInputEvent>(serde_json::Value::Object(map.clone())) {
                    return Ok(event);
                }
            }
        }
    }

    Err(format!("invalid remote input event: {text}"))
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[cfg(target_os = "macos")]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CGEventType {
    LeftMouseDown = 1,
    LeftMouseUp = 2,
    RightMouseDown = 3,
    RightMouseUp = 4,
    MouseMoved = 5,
    LeftMouseDragged = 6,
    RightMouseDragged = 7,
    OtherMouseDown = 25,
    OtherMouseUp = 26,
    OtherMouseDragged = 27,
}

#[cfg(target_os = "macos")]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CGMouseButton {
    Left = 0,
    Right = 1,
    Center = 2,
}

#[cfg(target_os = "macos")]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CGEventTapLocation {
    HIDEventTap = 0,
}

#[cfg(target_os = "macos")]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CGScrollEventUnit {
    Pixel = 0,
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> CGRect;
    fn CGEventCreateMouseEvent(
        source: *const std::ffi::c_void,
        mouseType: CGEventType,
        mouseCursorPosition: CGPoint,
        mouseButton: CGMouseButton,
    ) -> *mut std::ffi::c_void;
    fn CGEventCreateScrollWheelEvent2(
        source: *const std::ffi::c_void,
        units: CGScrollEventUnit,
        wheelCount: u32,
        wheel1: i32,
        wheel2: i32,
        wheel3: i32,
    ) -> *mut std::ffi::c_void;
    fn CGEventSetLocation(event: *mut std::ffi::c_void, location: CGPoint);
    fn CGEventSetIntegerValueField(event: *mut std::ffi::c_void, field: u32, value: i64);
    fn CGEventPost(tap: CGEventTapLocation, event: *mut std::ffi::c_void);
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const std::ffi::c_void);
}

/// Maps normalized coordinates [0.0..1.0] to global desktop coordinates.
pub fn map_normalized_coordinates(
    norm_x: f64,
    norm_y: f64,
    target: Option<&crate::webrtc::CaptureTarget>,
) -> (f64, f64) {
    let clamped_x = norm_x.clamp(0.0, 1.0);
    let clamped_y = norm_y.clamp(0.0, 1.0);

    #[cfg(target_os = "macos")]
    {
        if let Some(target) = target {
            if target.kind == crate::webrtc::CaptureKind::Screen {
                if let Some(ref source_id) = target.source_id {
                    if let Ok(display_id) = source_id.parse::<u32>() {
                        let bounds = unsafe { CGDisplayBounds(display_id) };
                        if bounds.size.width > 0.0 && bounds.size.height > 0.0 {
                            return (
                                bounds.origin.x + clamped_x * bounds.size.width,
                                bounds.origin.y + clamped_y * bounds.size.height,
                            );
                        }
                    }
                }
            } else if target.kind == crate::webrtc::CaptureKind::Window {
                if let Some(ref source_id) = target.source_id {
                    if let Ok(target_wid) = source_id.parse::<u32>() {
                        if let Ok(windows) = xcap::Window::all() {
                            if let Some(w) = windows.into_iter().find(|w| w.id().ok() == Some(target_wid)) {
                                if let (Ok(x), Ok(y), Ok(width), Ok(height)) = (w.x(), w.y(), w.width(), w.height()) {
                                    if width > 0 && height > 0 {
                                        return (
                                            x as f64 + clamped_x * width as f64,
                                            y as f64 + clamped_y * height as f64,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback to main display bounds
        let main_id = unsafe { CGMainDisplayID() };
        let bounds = unsafe { CGDisplayBounds(main_id) };
        if bounds.size.width > 0.0 && bounds.size.height > 0.0 {
            (
                bounds.origin.x + clamped_x * bounds.size.width,
                bounds.origin.y + clamped_y * bounds.size.height,
            )
        } else {
            (clamped_x * 1920.0, clamped_y * 1080.0)
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = target;
        (clamped_x * 1920.0, clamped_y * 1080.0)
    }
}

/// Injects synthetic input event into macOS WindowServer.
pub fn inject_remote_input(
    event: &RemoteInputEvent,
    target: Option<&crate::webrtc::CaptureTarget>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if !crate::permissions::check_accessibility_permission() {
            tracing::warn!("Accessibility permission not granted; remote input event dropped");
            return Err("Accessibility permission not granted".to_string());
        }

        let (norm_x, norm_y) = event.normalized_coords();
        let (target_x, target_y) = map_normalized_coordinates(norm_x, norm_y, target);
        let point = CGPoint {
            x: target_x,
            y: target_y,
        };

        match event {
            RemoteInputEvent::MouseMove { .. } => {
                let cg_event = unsafe {
                    CGEventCreateMouseEvent(
                        std::ptr::null(),
                        CGEventType::MouseMoved,
                        point,
                        CGMouseButton::Left,
                    )
                };
                if !cg_event.is_null() {
                    unsafe {
                        CGEventPost(CGEventTapLocation::HIDEventTap, cg_event);
                        CFRelease(cg_event);
                    }
                }
            }
            RemoteInputEvent::MouseDown { button, click_count, .. } => {
                let (event_type, mouse_button) = match button {
                    MouseButton::Left => (CGEventType::LeftMouseDown, CGMouseButton::Left),
                    MouseButton::Right => (CGEventType::RightMouseDown, CGMouseButton::Right),
                    MouseButton::Middle => (CGEventType::OtherMouseDown, CGMouseButton::Center),
                };
                let cg_event = unsafe {
                    CGEventCreateMouseEvent(
                        std::ptr::null(),
                        event_type,
                        point,
                        mouse_button,
                    )
                };
                if !cg_event.is_null() {
                    unsafe {
                        CGEventSetIntegerValueField(cg_event, 1, *click_count as i64);
                        CGEventPost(CGEventTapLocation::HIDEventTap, cg_event);
                        CFRelease(cg_event);
                    }
                }
            }
            RemoteInputEvent::MouseUp { button, click_count, .. } => {
                let (event_type, mouse_button) = match button {
                    MouseButton::Left => (CGEventType::LeftMouseUp, CGMouseButton::Left),
                    MouseButton::Right => (CGEventType::RightMouseUp, CGMouseButton::Right),
                    MouseButton::Middle => (CGEventType::OtherMouseUp, CGMouseButton::Center),
                };
                let cg_event = unsafe {
                    CGEventCreateMouseEvent(
                        std::ptr::null(),
                        event_type,
                        point,
                        mouse_button,
                    )
                };
                if !cg_event.is_null() {
                    unsafe {
                        CGEventSetIntegerValueField(cg_event, 1, *click_count as i64);
                        CGEventPost(CGEventTapLocation::HIDEventTap, cg_event);
                        CFRelease(cg_event);
                    }
                }
            }
            RemoteInputEvent::MouseClick { button, click_count, .. } => {
                let (down_type, up_type, mouse_button) = match button {
                    MouseButton::Left => (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, CGMouseButton::Left),
                    MouseButton::Right => (CGEventType::RightMouseDown, CGEventType::RightMouseUp, CGMouseButton::Right),
                    MouseButton::Middle => (CGEventType::OtherMouseDown, CGEventType::OtherMouseUp, CGMouseButton::Center),
                };
                let down_event = unsafe {
                    CGEventCreateMouseEvent(std::ptr::null(), down_type, point, mouse_button)
                };
                if !down_event.is_null() {
                    unsafe {
                        CGEventSetIntegerValueField(down_event, 1, *click_count as i64);
                        CGEventPost(CGEventTapLocation::HIDEventTap, down_event);
                        CFRelease(down_event);
                    }
                }
                let up_event = unsafe {
                    CGEventCreateMouseEvent(std::ptr::null(), up_type, point, mouse_button)
                };
                if !up_event.is_null() {
                    unsafe {
                        CGEventSetIntegerValueField(up_event, 1, *click_count as i64);
                        CGEventPost(CGEventTapLocation::HIDEventTap, up_event);
                        CFRelease(up_event);
                    }
                }
            }
            RemoteInputEvent::MouseDrag { button, .. } => {
                let (event_type, mouse_button) = match button {
                    MouseButton::Left => (CGEventType::LeftMouseDragged, CGMouseButton::Left),
                    MouseButton::Right => (CGEventType::RightMouseDragged, CGMouseButton::Right),
                    MouseButton::Middle => (CGEventType::OtherMouseDragged, CGMouseButton::Center),
                };
                let cg_event = unsafe {
                    CGEventCreateMouseEvent(
                        std::ptr::null(),
                        event_type,
                        point,
                        mouse_button,
                    )
                };
                if !cg_event.is_null() {
                    unsafe {
                        CGEventPost(CGEventTapLocation::HIDEventTap, cg_event);
                        CFRelease(cg_event);
                    }
                }
            }
            RemoteInputEvent::Scroll { delta_x, delta_y, .. } => {
                // In macOS Natural Scrolling, positive vertical delta scrolls down.
                let dy = (-delta_y) as i32;
                let dx = (-delta_x) as i32;
                let cg_event = unsafe {
                    CGEventCreateScrollWheelEvent2(
                        std::ptr::null(),
                        CGScrollEventUnit::Pixel,
                        2,
                        dy,
                        dx,
                        0,
                    )
                };
                if !cg_event.is_null() {
                    unsafe {
                        CGEventSetLocation(cg_event, point);
                        CGEventPost(CGEventTapLocation::HIDEventTap, cg_event);
                        CFRelease(cg_event);
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (event, target);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_direct_mouse_move_json() {
        let json = r#"{"action":"mouse_move","x":0.25,"y":0.75}"#;
        let event = parse_remote_input(json).expect("should parse");
        assert_eq!(event, RemoteInputEvent::MouseMove { x: 0.25, y: 0.75 });
    }

    #[test]
    fn parses_mouse_click_with_button() {
        let json = r#"{"action":"mouse_click","x":0.5,"y":0.5,"button":"right","click_count":2}"#;
        let event = parse_remote_input(json).expect("should parse");
        assert_eq!(
            event,
            RemoteInputEvent::MouseClick {
                x: 0.5,
                y: 0.5,
                button: MouseButton::Right,
                click_count: 2,
            }
        );
    }

    #[test]
    fn parses_scroll_event() {
        let json = r#"{"action":"scroll","x":0.5,"y":0.5,"delta_x":12.5,"delta_y":-30.0}"#;
        let event = parse_remote_input(json).expect("should parse");
        assert_eq!(
            event,
            RemoteInputEvent::Scroll {
                x: 0.5,
                y: 0.5,
                delta_x: 12.5,
                delta_y: -30.0,
            }
        );
    }

    #[test]
    fn parses_wrapped_remote_input_message() {
        let json = r#"{"type":"REMOTE_INPUT","payload":{"action":"mouse_drag","x":0.1,"y":0.9,"button":"left"}}"#;
        let event = parse_remote_input(json).expect("should parse");
        assert_eq!(
            event,
            RemoteInputEvent::MouseDrag {
                x: 0.1,
                y: 0.9,
                button: MouseButton::Left,
            }
        );
    }

    #[test]
    fn clamps_out_of_bounds_coordinates() {
        let (x1, y1) = map_normalized_coordinates(-0.5, 1.5, None);
        let (x2, y2) = map_normalized_coordinates(0.0, 1.0, None);
        assert_eq!(x1, x2);
        assert_eq!(y1, y2);
    }
}
