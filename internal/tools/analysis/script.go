package analysis

import (
	"fmt"
	"os"
	"path/filepath"
)

const envAnalyzeScript = "MAKIT_IFC_ANALYZE_SCRIPT"

func ResolveAnalyzeScript(customPath string) (string, error) {
	candidates := []string{}

	if customPath != "" {
		candidates = append(candidates, customPath)
	}

	if envPath := os.Getenv(envAnalyzeScript); envPath != "" {
		candidates = append(candidates, envPath)
	}

	if execPath, err := os.Executable(); err == nil {
		execDir := filepath.Dir(execPath)
		candidates = append(candidates,
			filepath.Join(execDir, "pyrevit-extension", "Makit.extension", "lib", "analyze_ifc.py"),
			filepath.Join(execDir, "..", "pyrevit-extension", "Makit.extension", "lib", "analyze_ifc.py"),
		)
	}

	candidates = append(candidates,
		"pyrevit-extension/Makit.extension/lib/analyze_ifc.py",
		"../pyrevit-extension/Makit.extension/lib/analyze_ifc.py",
	)

	for _, path := range candidates {
		absPath, err := filepath.Abs(path)
		if err != nil {
			continue
		}

		if _, err := os.Stat(absPath); err == nil {
			return absPath, nil
		}
	}

	return "", fmt.Errorf("analyze_ifc.py script not found; set --script or %s", envAnalyzeScript)
}
