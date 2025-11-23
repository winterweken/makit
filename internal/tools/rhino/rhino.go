package rhino

import (
	"fmt"

	"github.com/winteweken/makit/internal/registry"
)

// RegisterTasks registers all Rhino-related tasks
func RegisterTasks() {
	reg := registry.GetRegistry()
	tool := reg.RegisterTool("rhino", "Rhino 3D integration and automation")

	registerImportExportTasks(tool)
	registerGrasshopperTasks(tool)
}

func registerImportExportTasks(tool *registry.Tool) {
	category := tool.AddCategory("import-export", "Import and export data to/from Rhino")

	category.AddTask("import-revit", "Import Revit geometry to Rhino", func(ctx *registry.TaskContext) error {
		fmt.Println("Importing Revit geometry to Rhino...")
		return nil
	}).AddOption("input", "Input Revit file path", "string", true, nil).
		AddOption("layer", "Target layer name", "string", false, "Revit-Import")

	category.AddTask("export-model", "Export Rhino model to various formats", func(ctx *registry.TaskContext) error {
		fmt.Println("Exporting Rhino model...")
		return nil
	}).AddOption("format", "Export format (3dm, obj, fbx)", "string", true, nil).
		AddOption("output", "Output file path", "string", true, nil)
}

func registerGrasshopperTasks(tool *registry.Tool) {
	category := tool.AddCategory("grasshopper", "Grasshopper automation and scripting")

	category.AddTask("run-definition", "Run a Grasshopper definition", func(ctx *registry.TaskContext) error {
		fmt.Println("Running Grasshopper definition...")
		return nil
	}).AddOption("definition", "Path to Grasshopper definition (.gh)", "string", true, nil).
		AddOption("headless", "Run in headless mode", "bool", false, true)

	category.AddTask("bake-geometry", "Bake Grasshopper geometry to Rhino", func(ctx *registry.TaskContext) error {
		fmt.Println("Baking Grasshopper geometry...")
		return nil
	}).AddOption("layer", "Target layer name", "string", false, "GH-Bake")
}
