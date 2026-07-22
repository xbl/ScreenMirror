use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize)]
struct StoredData {
    entries: HashMap<String, String>,
}

pub struct Storage {
    path: PathBuf,
    data: StoredData,
}

impl Storage {
    pub fn open() -> Result<Self, String> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("config.json");
        let data = if path.exists() {
            let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            StoredData::default()
        };
        Ok(Self { path, data })
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.data.entries.get(key).cloned()
    }

    pub fn set_string(&mut self, key: &str, value: &str) {
        self.data.entries.insert(key.to_string(), value.to_string());
        let _ = self.flush();
    }

    pub fn delete(&mut self, key: &str) {
        self.data.entries.remove(key);
        let _ = self.flush();
    }

    fn flush(&self) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, raw).map_err(|e| e.to_string())
    }
}

fn config_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(custom).join("screenmirror");
    }
    let proj = directories::ProjectDirs::from("dev", "screenmirror", "screenmirror");
    match proj {
        Some(p) => p.config_dir().to_path_buf(),
        None => std::env::temp_dir().join("screenmirror"),
    }
}
