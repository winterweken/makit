package config

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/viper"
)

type Config struct {
	PyRevit PyRevitConfig `mapstructure:"pyrevit"`
	General GeneralConfig `mapstructure:"general"`
}

type PyRevitConfig struct {
	InstallPath    string   `mapstructure:"install_path"`
	ExtensionsPaths []string `mapstructure:"extensions_paths"`
	DefaultRevitVersion string `mapstructure:"default_revit_version"`
}

type GeneralConfig struct {
	Editor      string `mapstructure:"editor"`
	AutoUpdate  bool   `mapstructure:"auto_update"`
	LogLevel    string `mapstructure:"log_level"`
}

func Initialize() error {
	home, err := os.UserHomeDir()
	if err != nil {
		return fmt.Errorf("failed to get home directory: %w", err)
	}

	configPath := filepath.Join(home, ".makit.yaml")

	if _, err := os.Stat(configPath); err == nil {
		return fmt.Errorf("config file already exists at %s", configPath)
	}

	defaultConfig := `# Makit Configuration File
pyrevit:
  install_path: ""
  extensions_paths:
    - "~/pyRevit/extensions"
  default_revit_version: "2024"

general:
  editor: "code"
  auto_update: true
  log_level: "info"
`

	if err := os.WriteFile(configPath, []byte(defaultConfig), 0644); err != nil {
		return fmt.Errorf("failed to write config file: %w", err)
	}

	return nil
}

func Load() (*Config, error) {
	var cfg Config

	if err := viper.Unmarshal(&cfg); err != nil {
		return nil, fmt.Errorf("failed to unmarshal config: %w", err)
	}

	return &cfg, nil
}

func GetPyRevitPath() string {
	return viper.GetString("pyrevit.install_path")
}

func GetEditor() string {
	editor := viper.GetString("general.editor")
	if editor == "" {
		editor = os.Getenv("EDITOR")
		if editor == "" {
			editor = "vim"
		}
	}
	return editor
}
