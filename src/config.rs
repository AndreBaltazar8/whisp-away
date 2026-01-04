use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// System configuration (set by NixOS module or manually)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default = "default_model")]
    pub default_model: String,

    #[serde(default = "default_backend")]
    pub default_backend: String,

    #[serde(default)]
    pub use_clipboard: bool,
}

fn default_model() -> String {
    "base.en".to_string()
}

fn default_backend() -> String {
    "faster-whisper".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_model: default_model(),
            default_backend: default_backend(),
            use_clipboard: false,
        }
    }
}

/// Get the user config file path
pub fn get_config_path() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            PathBuf::from(home).join(".config")
        });

    config_dir.join("whisp-away").join("config.json")
}

/// Get the system-wide config file path (NixOS)
fn get_system_config_path() -> PathBuf {
    PathBuf::from("/etc/xdg/whisp-away/config.json")
}

/// Read config if available, checking user config first, then system config
pub fn read_config() -> Option<Config> {
    // Try user config first
    let user_config_path = get_config_path();
    if let Ok(content) = std::fs::read_to_string(&user_config_path) {
        if let Ok(config) = serde_json::from_str(&content) {
            return Some(config);
        }
    }

    // Fall back to system config
    let system_config_path = get_system_config_path();
    if let Ok(content) = std::fs::read_to_string(&system_config_path) {
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

/// Write config file
pub fn write_config(config: &Config) -> Result<()> {
    let config_path = get_config_path();

    // Ensure config dir exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(config_path, json)?;
    Ok(())
}
