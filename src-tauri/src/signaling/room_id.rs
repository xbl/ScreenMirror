use rand::RngCore;
use std::collections::HashSet;

pub struct RoomIDService {
    taken: HashSet<String>,
}

impl RoomIDService {
    pub fn new() -> Self {
        Self {
            taken: HashSet::new(),
        }
    }

    pub fn get_simple_available_room_id(&self) -> String {
        loop {
            let mut buf = [0u8; 4];
            rand::thread_rng().fill_bytes(&mut buf);
            let n = u32::from_be_bytes(buf) % 1_000_000;
            let id = format!("{:06}", n);
            if !self.taken.contains(&id) {
                return id;
            }
        }
    }

    pub fn mark_taken(&mut self, id: &str) {
        self.taken.insert(id.to_string());
    }

    pub fn unmark_taken(&mut self, id: &str) {
        self.taken.remove(id);
    }

    pub fn is_taken(&self, id: &str) -> bool {
        self.taken.contains(id)
    }
}

impl Default for RoomIDService {
    fn default() -> Self {
        Self::new()
    }
}
