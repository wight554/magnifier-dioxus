use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub default_zoom: f32,
    pub torch_on_launch: bool,
    #[serde(default)]
    pub use_macro: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_zoom: 2.0,
            torch_on_launch: false,
            use_macro: false,
        }
    }
}

pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(s)?)
}

pub fn settings_path() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        crate::camera::app_files_dir().join("settings.json")
    }
    #[cfg(not(target_os = "android"))]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("magnifier/settings.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join("magnifier-test-rt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        let s = Settings {
            default_zoom: 3.5,
            torch_on_launch: true,
            use_macro: true,
        };
        save(&p, &s).unwrap();
        assert_eq!(load(&p), s);
    }

    #[test]
    fn missing_file_gives_default() {
        assert_eq!(
            load(std::path::Path::new("/nonexistent/x.json")),
            Settings::default()
        );
    }

    #[test]
    fn corrupt_file_gives_default() {
        let dir = std::env::temp_dir().join("magnifier-test-corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(load(&p), Settings::default());
    }

    #[test]
    fn missing_use_macro_key_defaults_false() {
        let dir = std::env::temp_dir().join("magnifier-test-old-format");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, r#"{"default_zoom":4.0,"torch_on_launch":true}"#).unwrap();
        let loaded = load(&p);
        assert_eq!(loaded.default_zoom, 4.0);
        assert!(loaded.torch_on_launch);
        assert!(!loaded.use_macro);
    }
}
