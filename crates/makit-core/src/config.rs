//! Configuration management using figment.

use figment::{Figment, providers::{Format, Yaml, Env}};
use serde::Deserialize;

/// Main configuration structure.
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub pyrevit: PyRevitConfig,
    #[serde(default)]
    pub general: GeneralConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct PyRevitConfig {
    #[serde(default)]
    pub install_path: String,
    #[serde(default)]
    pub extensions_paths: Vec<String>,
    #[serde(default = "default_revit_version")]
    pub default_revit_version: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct GeneralConfig {
    #[serde(default = "default_editor")]
    pub editor: String,
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_revit_version() -> String {
    "2024".to_string()
}

fn default_editor() -> String {
    "code".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Config {
    /// Load configuration from ~/.makit.yaml, falling back to defaults.
    pub fn load(config_file: Option<&str>) -> anyhow::Result<Self> {
        let mut figment = Figment::new();

        if let Some(path) = config_file {
            figment = figment.merge(Yaml::file(path));
        } else if let Some(home) = dirs_home() {
            let default_path = format!("{}/.makit.yaml", home);
            figment = figment.merge(Yaml::file(default_path));
        }

        figment = figment.merge(Env::prefixed("MAKIT_"));

        let config: Config = figment.extract().unwrap_or_default();
        Ok(config)
    }
}

fn dirs_home() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}
