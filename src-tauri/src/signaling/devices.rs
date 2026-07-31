use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub os: String,
    pub browser: String,
    pub room_id: String,
    pub sharing_session_id: String,
}

pub struct ConnectedDevicesService {
    devices: Vec<Device>,
}

impl ConnectedDevicesService {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: Device) -> Result<(), String> {
        if let Some(existing) = self.devices.iter_mut().find(|d| d.id == device.id) {
            *existing = device;
        } else {
            self.devices.push(device);
        }
        Ok(())
    }

    pub fn release_device(&mut self, id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|device| device.id != id);
        self.devices.len() != before
    }

    pub fn release_all(&mut self) {
        self.devices.clear();
    }

    pub fn get_devices(&self) -> Vec<Device> {
        self.devices.clone()
    }

    pub fn is_slot_available(&self) -> bool {
        true
    }

    pub fn set_pending(&mut self, device: Device) {
        self.add_device(device).ok();
    }

    pub fn reset_pending(&mut self) {
        // Kept for command/API compatibility; connections are now independent.
    }

    pub fn get_pending(&self) -> Option<Device> {
        None
    }
}

impl Default for ConnectedDevicesService {
    fn default() -> Self {
        Self::new()
    }
}
