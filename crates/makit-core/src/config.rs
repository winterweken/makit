//! Configuration management using figment.

use figment::{
    providers::{Env, Format, Yaml},
    Figment,
};
use serde::Deserialize;

/// Main configuration structure.
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub pyrevit: PyRevitConfig,
    #[serde(default)]
    pub general: GeneralConfig,
}

#[derive(Debug, Deserialize)]
pub struct PyRevitConfig {
    #[serde(default)]
    pub install_path: String,
    #[serde(default)]
    pub extensions_paths: Vec<String>,
    #[serde(default = "default_revit_version")]
    pub default_revit_version: String,
}

#[derive(Debug, Deserialize)]
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

impl Default for PyRevitConfig {
    fn default() -> Self {
        Self {
            install_path: String::new(),
            extensions_paths: Vec::new(),
            default_revit_version: default_revit_version(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            editor: default_editor(),
            auto_update: false,
            log_level: default_log_level(),
        }
    }
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

        let config: Config = figment.extract().map_err(|e| anyhow::anyhow!("Config parsing error: {}", e))?;
        Ok(config)
    }
}

fn dirs_home() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config: Config = Figment::new().merge(Yaml::string("")).extract().unwrap();
        assert_eq!(config.general.editor, "code");
        assert_eq!(config.pyrevit.default_revit_version, "2024");
        assert_eq!(config.general.log_level, "info");
    }

    #[test]
    fn test_valid_config() {
        let path = "test_valid_config.yaml";
        std::fs::write(path, "general:\n  editor: vim").unwrap();
        let config = Config::load(Some(path)).unwrap();
        assert_eq!(config.general.editor, "vim");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_invalid_config() {
        let path = "test_invalid_config.yaml";
        std::fs::write(path, "general:\n  editor: [unclosed list").unwrap();
        let result = Config::load(Some(path));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Config parsing error"));
        std::fs::remove_file(path).ok();
    }
}
