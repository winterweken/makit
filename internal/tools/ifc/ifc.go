package ifc

import (
	"fmt"
	"os"

	"github.com/winteweken/makit/internal/registry"
)

// RegisterTasks registers IFC-related components
func RegisterTasks() {
	reg := registry.GetRegistry()

	reg.RegisterSource("ifc", "Industry Foundation Classes file", handleLoadIFC).
		AddOption("file", "Path to IFC file", "string", true, nil)
}

func handleLoadIFC(ctx *registry.TaskContext) error {
	path, ok := ctx.Options["file"].(string)
	if !ok || path == "" {
		return fmt.Errorf("file path is required")
	}

	if _, err := os.Stat(path); os.IsNotExist(err) {
		return fmt.Errorf("file not found: %s", path)
	}

	fmt.Printf("Connected to IFC source: %s\n", path)
	return nil
}
