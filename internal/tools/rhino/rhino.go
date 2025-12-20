package rhino

import (
	"fmt"

	"github.com/winteweken/makit/internal/registry"
)

// RegisterTasks registers all Rhino-related tasks
// RegisterTasks registers all Rhino-related tasks
func RegisterTasks() {
	reg := registry.GetRegistry()

	// Register Rhino as a Source
	reg.RegisterSource("rhino", "Rhino 3D integration", func(ctx *registry.TaskContext) error {
		fmt.Println("Connected to Rhino...")
		return nil
	})

	// Import/Export Actions
	reg.RegisterAction("rhino-import-revit", "Import Revit geometry to Rhino", "import-export", func(ctx *registry.TaskContext) error {
		fmt.Println("Importing Revit geometry to Rhino...")
		return nil
	}).AddOption("input", "Input Revit file path", "string", true, nil).
		AddOption("layer", "Target layer name", "string", false, "Revit-Import")

	reg.RegisterAction("rhino-export-model", "Export Rhino model", "import-export", func(ctx *registry.TaskContext) error {
		fmt.Println("Exporting Rhino model...")
		return nil
	}).AddOption("format", "Export format (3dm, obj, fbx)", "string", true, nil).
		AddOption("output", "Output file path", "string", true, nil)

	// Grasshopper Actions
	reg.RegisterAction("run-gh-definition", "Run a Grasshopper definition", "grasshopper", func(ctx *registry.TaskContext) error {
		fmt.Println("Running Grasshopper definition...")
		return nil
	}).AddOption("definition", "Path to Grasshopper definition (.gh)", "string", true, nil).
		AddOption("headless", "Run in headless mode", "bool", false, true)

	reg.RegisterAction("gh-bake", "Bake Grasshopper geometry to Rhino", "grasshopper", func(ctx *registry.TaskContext) error {
		fmt.Println("Baking Grasshopper geometry...")
		return nil
	}).AddOption("layer", "Target layer name", "string", false, "GH-Bake")
}
