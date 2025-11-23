package revit

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/winteweken/makit/internal/pyrevit"
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

		client := pyrevit.NewClient("")

		// Check connection
		if err := client.HealthCheck(); err != nil {
			return fmt.Errorf("pyRevit server not available: %w\nMake sure pyRevit extension is running in Revit", err)
		}

		// Extract walls
		options := pyrevit.WallExtractionOptions{
			IncludeCurved:  true,
			IncludeStacked: true,
		}

		result, err := client.ExtractWalls(options)
		if err != nil {
			return fmt.Errorf("failed to extract walls: %w", err)
		}

		fmt.Printf("Extracted %d walls\n", result.Count)

		// Save to file if output specified
		if output, ok := ctx.Options["output"].(string); ok {
			data, _ := json.MarshalIndent(result, "", "  ")
			if err := os.WriteFile(output, data, 0644); err != nil {
				return fmt.Errorf("failed to write output: %w", err)
			}
			fmt.Printf("Saved to %s\n", output)
		}

		return nil
	}).AddOption("output", "Output file path", "string", false, "walls.json").
		AddOption("level", "Filter by level name", "string", false, "")

	category.AddTask("extract-floors", "Extract floor elements from a Revit model", func(ctx *registry.TaskContext) error {
		fmt.Println("Extracting floors from Revit model...")

		client := pyrevit.NewClient("")

		if err := client.HealthCheck(); err != nil {
			return fmt.Errorf("pyRevit server not available: %w", err)
		}

		options := pyrevit.FloorExtractionOptions{}

		result, err := client.ExtractFloors(options)
		if err != nil {
			return fmt.Errorf("failed to extract floors: %w", err)
		}

		fmt.Printf("Extracted %d floors\n", result.Count)

		if output, ok := ctx.Options["output"].(string); ok {
			data, _ := json.MarshalIndent(result, "", "  ")
			if err := os.WriteFile(output, data, 0644); err != nil {
				return fmt.Errorf("failed to write output: %w", err)
			}
			fmt.Printf("Saved to %s\n", output)
		}

		return nil
	}).AddOption("output", "Output file path", "string", false, "floors.json")

	category.AddTask("extract-rooms", "Extract room elements from a Revit model", func(ctx *registry.TaskContext) error {
		fmt.Println("Extracting rooms from Revit model...")

		client := pyrevit.NewClient("")

		if err := client.HealthCheck(); err != nil {
			return fmt.Errorf("pyRevit server not available: %w", err)
		}

		options := pyrevit.RoomExtractionOptions{
			IncludeUnplaced: false,
		}

		result, err := client.ExtractRooms(options)
		if err != nil {
			return fmt.Errorf("failed to extract rooms: %w", err)
		}

		fmt.Printf("Extracted %d rooms\n", result.Count)
		for _, room := range result.Rooms {
			fmt.Printf("  - %s (%s): %.2f SF\n", room.Name, room.Number, room.Area)
		}

		if output, ok := ctx.Options["output"].(string); ok {
			data, _ := json.MarshalIndent(result, "", "  ")
			if err := os.WriteFile(output, data, 0644); err != nil {
				return fmt.Errorf("failed to write output: %w", err)
			}
			fmt.Printf("Saved to %s\n", output)
		}

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
