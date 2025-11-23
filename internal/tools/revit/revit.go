package revit

import (
	"fmt"

	"github.com/winteweken/makit/internal/registry"
)

// RegisterTasks registers all Revit-related tasks
func RegisterTasks() {
	reg := registry.GetRegistry()
	tool := reg.RegisterTool("revit", "Autodesk Revit integration and automation")

	registerGeometryTasks(tool)
	registerAnalysisTasks(tool)
	registerModificationTasks(tool)
}

func registerGeometryTasks(tool *registry.Tool) {
	category := tool.AddCategory("geometry", "Extract and manipulate geometric elements")

	category.AddTask("extract-walls", "Extract wall elements from a Revit model", func(ctx *registry.TaskContext) error {
		fmt.Println("Extracting walls from Revit model...")
		return nil
	}).AddOption("output", "Output file path", "string", false, "walls.json")

	category.AddTask("extract-floors", "Extract floor elements from a Revit model", func(ctx *registry.TaskContext) error {
		fmt.Println("Extracting floors from Revit model...")
		return nil
	}).AddOption("output", "Output file path", "string", false, "floors.json")

	category.AddTask("extract-rooms", "Extract room elements from a Revit model", func(ctx *registry.TaskContext) error {
		fmt.Println("Extracting rooms from Revit model...")
		return nil
	}).AddOption("output", "Output file path", "string", false, "rooms.json")
}

func registerAnalysisTasks(tool *registry.Tool) {
	category := tool.AddCategory("analysis", "Analyze Revit models and elements")

	category.AddTask("calculate-areas", "Calculate areas of rooms and spaces", func(ctx *registry.TaskContext) error {
		fmt.Println("Calculating areas...")
		return nil
	}).AddOption("unit", "Area unit (sqft, sqm)", "string", false, "sqft")

	category.AddTask("find-clashes", "Detect clashes between elements", func(ctx *registry.TaskContext) error {
		fmt.Println("Finding clashes...")
		return nil
	}).AddOption("tolerance", "Clash detection tolerance", "float", false, 0.01)

	category.AddTask("validate-standards", "Validate model against standards", func(ctx *registry.TaskContext) error {
		fmt.Println("Validating standards...")
		return nil
	}).AddOption("ruleset", "Path to validation ruleset", "string", true, nil)
}

func registerModificationTasks(tool *registry.Tool) {
	category := tool.AddCategory("modification", "Modify Revit model elements")

	category.AddTask("update-parameters", "Update element parameters", func(ctx *registry.TaskContext) error {
		fmt.Println("Updating parameters...")
		return nil
	}).AddOption("parameter", "Parameter name", "string", true, nil).
		AddOption("value", "New parameter value", "string", true, nil)

	category.AddTask("apply-templates", "Apply templates to elements", func(ctx *registry.TaskContext) error {
		fmt.Println("Applying templates...")
		return nil
	}).AddOption("template", "Path to template file", "string", true, nil)
}
