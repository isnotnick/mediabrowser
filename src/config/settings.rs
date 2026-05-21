use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Result, Context};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub server_port: u16,
    pub media_directory: PathBuf,
    pub optional_password: Option<String>,
    pub thumbnail_size: u32,
    pub thumbnail_quality: u8,
    pub generate_timeline_thumbnails: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_port: 8080,
            media_directory: PathBuf::from("./media"),
            optional_password: None,
            thumbnail_size: 320,
            thumbnail_quality: 60,
            generate_timeline_thumbnails: false,
        }
    }
}

impl Settings {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path).context("Failed to read config file")?;
            let settings: Settings = serde_yaml::from_str(&content).context("Failed to parse config file")?;
            Ok(settings)
        } else {
            let settings = Settings::default();
            let content = serde_yaml::to_string(&settings).context("Failed to serialize default config")?;
            fs::write(path, content).context("Failed to write default config file")?;
            Ok(settings)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_yaml::to_string(self).context("Failed to serialize config")?;
        fs::write(path, content).context("Failed to write config file")?;
        Ok(())
    }
}
