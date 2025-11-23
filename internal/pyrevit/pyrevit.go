package pyrevit

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

type Status struct {
	Installed   bool
	Version     string
	InstallPath string
	Extensions  []Extension
}

type Extension struct {
	Name    string
	Version string
	Path    string
	Enabled bool
}

func GetStatus() (*Status, error) {
	status := &Status{
		Installed: false,
	}

	installPath, err := findPyRevitInstallation()
	if err != nil {
		return status, nil
	}

	status.Installed = true
	status.InstallPath = installPath

	version, err := getPyRevitVersion(installPath)
	if err == nil {
		status.Version = version
	}

	extensions, err := getExtensions(installPath)
	if err == nil {
		status.Extensions = extensions
	}

	return status, nil
}

func findPyRevitInstallation() (string, error) {
	var searchPaths []string

	if runtime.GOOS == "windows" {
		appData := os.Getenv("APPDATA")
		programData := os.Getenv("PROGRAMDATA")

		searchPaths = []string{
			filepath.Join(appData, "pyRevit"),
			filepath.Join(programData, "pyRevit"),
			filepath.Join(os.Getenv("USERPROFILE"), "pyRevit"),
		}
	} else {
		home, _ := os.UserHomeDir()
		searchPaths = []string{
			filepath.Join(home, ".pyRevit"),
			filepath.Join(home, "pyRevit"),
		}
	}

	for _, path := range searchPaths {
		if _, err := os.Stat(path); err == nil {
			return path, nil
		}
	}

	return "", fmt.Errorf("pyRevit installation not found")
}

func getPyRevitVersion(installPath string) (string, error) {
	return "4.8.x", nil
}

func getExtensions(installPath string) ([]Extension, error) {
	extensionsPath := filepath.Join(installPath, "extensions")

	if _, err := os.Stat(extensionsPath); os.IsNotExist(err) {
		return []Extension{}, nil
	}

	entries, err := os.ReadDir(extensionsPath)
	if err != nil {
		return nil, err
	}

	extensions := []Extension{}
	for _, entry := range entries {
		if entry.IsDir() {
			ext := Extension{
				Name:    entry.Name(),
				Path:    filepath.Join(extensionsPath, entry.Name()),
				Enabled: true,
			}
			extensions = append(extensions, ext)
		}
	}

	return extensions, nil
}

func RunScript(scriptPath string) error {
	if _, err := os.Stat(scriptPath); os.IsNotExist(err) {
		return fmt.Errorf("script not found: %s", scriptPath)
	}

	cmd := exec.Command("python", scriptPath)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	return cmd.Run()
}
