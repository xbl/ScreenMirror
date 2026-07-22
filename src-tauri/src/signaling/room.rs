use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub is_locked: bool,
    pub owner_socket_id: Option<String>,
    pub viewer_socket_id: Option<String>,
    pub updated_at: i64,
}

impl Room {
    pub fn new(id: String) -> Self {
        Self {
            id,
            is_locked: false,
            owner_socket_id: None,
            viewer_socket_id: None,
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn toggle_lock(&mut self) {
        self.is_locked = !self.is_locked;
    }
}
