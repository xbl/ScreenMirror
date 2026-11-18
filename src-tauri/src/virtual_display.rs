use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualDisplayConfig {
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: Option<f64>,
    pub hi_dpi: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VirtualDisplayInfo {
    pub display_id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_virtual: bool,
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::ffi::CString;

    extern "C" {
        fn sm_virtual_display_is_supported() -> bool;
        fn sm_virtual_display_create(
            name: *const std::os::raw::c_char,
            width: u32,
            height: u32,
            refresh_rate: f64,
            hi_dpi: u32,
        ) -> u32;
        fn sm_virtual_display_destroy(display_id: u32) -> bool;
        fn sm_virtual_display_destroy_all();
        fn sm_virtual_display_get_active_ids(out_ids: *mut u32, max_count: u32) -> u32;
    }

    pub fn is_supported() -> bool {
        unsafe { sm_virtual_display_is_supported() }
    }

    pub fn create_display(config: &VirtualDisplayConfig) -> Result<u32, String> {
        if !is_supported() {
            return Err("CGVirtualDisplay is not supported on this macOS version".into());
        }

        let name_str = config
            .name
            .clone()
            .unwrap_or_else(|| "ScreenMirror Extended Display".into());
        let c_name = CString::new(name_str).map_err(|e| e.to_string())?;

        let width = if config.width == 0 { 1920 } else { config.width };
        let height = if config.height == 0 { 1080 } else { config.height };
        let refresh_rate = config.refresh_rate.unwrap_or(60.0);
        let hi_dpi = if config.hi_dpi.unwrap_or(false) { 1 } else { 0 };

        let display_id = unsafe {
            sm_virtual_display_create(c_name.as_ptr(), width, height, refresh_rate, hi_dpi)
        };

        if display_id == 0 {
            Err("Failed to create virtual display via CGVirtualDisplay".into())
        } else {
            Ok(display_id)
        }
    }

    pub fn destroy_display(display_id: u32) -> Result<(), String> {
        let destroyed = unsafe { sm_virtual_display_destroy(display_id) };
        if destroyed {
            Ok(())
        } else {
            Err(format!("Virtual display {display_id} not found or already destroyed"))
        }
    }

    pub fn destroy_all() {
        unsafe { sm_virtual_display_destroy_all() };
    }

    pub fn get_active_ids() -> Vec<u32> {
        let mut ids = [0u32; 16];
        let count = unsafe { sm_virtual_display_get_active_ids(ids.as_mut_ptr(), 16) };
        let valid_len = (count as usize).min(16);
        ids[..valid_len].to_vec()
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::*;

    pub fn is_supported() -> bool {
        false
    }

    pub fn create_display(_config: &VirtualDisplayConfig) -> Result<u32, String> {
        Err("Virtual display is only supported on macOS".into())
    }

    pub fn destroy_display(_display_id: u32) -> Result<(), String> {
        Ok(())
    }

    pub fn destroy_all() {}

    pub fn get_active_ids() -> Vec<u32> {
        Vec::new()
    }
}

pub fn is_supported() -> bool {
    native::is_supported()
}

pub fn create_display(config: &VirtualDisplayConfig) -> Result<u32, String> {
    native::create_display(config)
}

pub fn destroy_display(display_id: u32) -> Result<(), String> {
    native::destroy_display(display_id)
}

pub fn destroy_all() {
    native::destroy_all();
}

pub fn get_active_ids() -> Vec<u32> {
    native::get_active_ids()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_virtual_display_support_and_lifecycle() {
        if !is_supported() {
            println!("CGVirtualDisplay not supported in this test environment, skipping lifecycle test");
            return;
        }

        let config = VirtualDisplayConfig {
            name: Some("Test Virtual Display".into()),
            width: 1280,
            height: 720,
            refresh_rate: Some(60.0),
            hi_dpi: Some(false),
        };

        let display_id = create_display(&config).expect("create_display should succeed");
        assert!(display_id > 0, "display_id must be non-zero");

        let active_ids = get_active_ids();
        assert!(
            active_ids.contains(&display_id),
            "active_ids should contain the newly created display"
        );

        destroy_display(display_id).expect("destroy_display should succeed");

        let after_destroy = get_active_ids();
        assert!(
            !after_destroy.contains(&display_id),
            "display should be removed after destroy"
        );
    }
}
