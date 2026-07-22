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
    occupied: Option<Device>,
    pending: Option<Device>,
}

impl ConnectedDevicesService {
    pub fn new() -> Self {
        Self {
            occupied: None,
            pending: None,
        }
    }

    pub fn add_device(&mut self, device: Device) -> Result<(), String> {
        match &self.occupied {
            Some(d) if d.id == device.id => Ok(()),
            Some(_) => Err("viewer slot is already occupied".into()),
            None => {
                self.occupied = Some(device);
                Ok(())
            }
        }
    }

    pub fn release_device(&mut self, id: &str) -> bool {
        match &self.occupied {
            Some(d) if d.id == id => {
                self.occupied = None;
                true
            }
            _ => false,
        }
    }

    pub fn get_devices(&self) -> Vec<Device> {
        self.occupied.clone().into_iter().collect()
    }

    pub fn is_slot_available(&self) -> bool {
        self.occupied.is_none()
    }

    pub fn set_pending(&mut self, device: Device) {
        self.pending = Some(device);
    }

    pub fn reset_pending(&mut self) {
        self.pending = None;
    }

    pub fn get_pending(&self) -> Option<Device> {
        self.pending.clone()
    }
}

impl Default for ConnectedDevicesService {
    fn default() -> Self {
        Self::new()
    }
}
