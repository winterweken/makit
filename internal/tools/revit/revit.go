package revit

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/winteweken/makit/internal/pyrevit"
	"github.com/winteweken/makit/internal/registry"
)

// RegisterTasks registers all Revit-related tasks
// RegisterTasks registers all Revit-related tasks
func RegisterTasks() {
	reg := registry.GetRegistry()

	// Register Revit as a Source
	// The default handler extracts the whole model
	reg.RegisterSource("revit", "Autodesk Revit integration", handleExtractModel).
		AddOption("workset", "Filter by workset name", "string", false, "").
		AddOption("wall-type", "Filter by wall type", "string", false, "").
		AddOption("output", "Output file path", "string", false, "building-model.json")

	// Register specific Geometry Tasks as "Specialized Sources"? Or just Actions?
	// Let's make them Actions that fetch from Revit
	reg.RegisterAction("revit-extract-walls", "Extract wall elements from Revit", "extraction", handleExtractWalls).
		AddOption("output", "Output file path", "string", false, "walls.json").
		AddOption("level", "Filter by level name", "string", false, "")

	reg.RegisterAction("revit-extract-floors", "Extract floor elements from Revit", "extraction", handleExtractFloors).
		AddOption("output", "Output file path", "string", false, "floors.json")

	reg.RegisterAction("revit-extract-rooms", "Extract room elements from Revit", "extraction", handleExtractRooms).
		AddOption("output", "Output file path", "string", false, "rooms.json")

	// Analysis Actions
	reg.RegisterAction("revit-wall-orientations", "Analyze wall orientations in Revit", "analysis", handleWallOrientations).
		AddOption("workset", "Filter by workset name", "string", false, "").
		AddOption("wall-type", "Filter by wall type (e.g., Exterior)", "string", false, "").
		AddOption("unit", "Area unit (sqm, sqf)", "string", false, "sqm").
		AddOption("output", "Save detailed JSON results to file", "string", false, "")

	reg.RegisterAction("revit-calculate-areas", "Calculate areas of rooms/spaces", "analysis", handleCalculateAreas).
		AddOption("unit", "Area unit (sqft, sqm)", "string", false, "sqft")

	reg.RegisterAction("revit-find-clashes", "Detect clashes in Revit", "analysis", handleFindClashes).
		AddOption("tolerance", "Clash detection tolerance", "float", false, 0.01)

	reg.RegisterAction("revit-validate-standards", "Validate Revit model against standards", "analysis", handleValidateStandards).
		AddOption("ruleset", "Path to validation ruleset", "string", true, nil)

	// Modification Actions
	reg.RegisterAction("revit-update-parameters", "Update element parameters in Revit", "modification", handleUpdateParameters).
		AddOption("parameter", "Parameter name", "string", true, nil).
		AddOption("value", "New parameter value", "string", true, nil)

	reg.RegisterAction("revit-apply-templates", "Apply templates to elements in Revit", "modification", handleApplyTemplates).
		AddOption("template", "Path to template file", "string", true, nil)
}

// Helpers

func getClient() (*pyrevit.Client, error) {
	client := pyrevit.NewClient("")
	if err := client.HealthCheck(); err != nil {
		return nil, fmt.Errorf("pyRevit server not available: %w\nMake sure pyRevit extension is running in Revit", err)
	}
	return client, nil
}

func saveJSON(data interface{}, path string) error {
	bytes, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal result: %w", err)
	}
	if err := os.WriteFile(path, bytes, 0644); err != nil {
		return fmt.Errorf("failed to write output: %w", err)
	}
	fmt.Printf("Saved to %s\n", path)
	return nil
}

// Handlers

func handleExtractWalls(ctx *registry.TaskContext) error {
	fmt.Println("Extracting walls from Revit model...")

	client, err := getClient()
	if err != nil {
		return err
	}

	options := pyrevit.WallExtractionOptions{
		IncludeCurved:  true,
		IncludeStacked: true,
	}

	result, err := client.ExtractWalls(options)
	if err != nil {
		return fmt.Errorf("failed to extract walls: %w", err)
	}

	fmt.Printf("Extracted %d walls\n", result.Count)

	if output, ok := ctx.Options["output"].(string); ok {
		if err := saveJSON(result, output); err != nil {
			return err
		}
	}
	return nil
}

func handleExtractFloors(ctx *registry.TaskContext) error {
	fmt.Println("Extracting floors from Revit model...")

	client, err := getClient()
	if err != nil {
		return err
	}

	options := pyrevit.FloorExtractionOptions{}

	result, err := client.ExtractFloors(options)
	if err != nil {
		return fmt.Errorf("failed to extract floors: %w", err)
	}

	fmt.Printf("Extracted %d floors\n", result.Count)

	if output, ok := ctx.Options["output"].(string); ok {
		if err := saveJSON(result, output); err != nil {
			return err
		}
	}
	return nil
}

func handleExtractRooms(ctx *registry.TaskContext) error {
	fmt.Println("Extracting rooms from Revit model...")

	client, err := getClient()
	if err != nil {
		return err
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
		if err := saveJSON(result, output); err != nil {
			return err
		}
	}
	return nil
}

func handleWallOrientations(ctx *registry.TaskContext) error {
	fmt.Println("Analyzing wall orientations...")

	client, err := getClient()
	if err != nil {
		return err
	}

	options := pyrevit.WallOrientationOptions{
		IncludeWindows: true,
		AreaUnit:       "sqm",
	}

	if workset, ok := ctx.Options["workset"].(string); ok && workset != "" {
		options.Workset = workset
	}
	if wallType, ok := ctx.Options["wall-type"].(string); ok && wallType != "" {
		options.WallType = wallType
	}
	if areaUnit, ok := ctx.Options["unit"].(string); ok && areaUnit != "" {
		options.AreaUnit = areaUnit
	}

	result, err := client.AnalyzeWallOrientations(options)
	if err != nil {
		return fmt.Errorf("failed to analyze wall orientations: %w", err)
	}

	fmt.Println(result.Report)

	if output, ok := ctx.Options["output"].(string); ok && output != "" {
		if err := saveJSON(result, output); err != nil {
			return err
		}
		fmt.Printf("\nDetailed results saved to %s\n", output)
	}
	return nil
}

func handleExtractModel(ctx *registry.TaskContext) error {
	fmt.Println("Extracting building model to generic format...")

	client, err := getClient()
	if err != nil {
		return err
	}

	options := pyrevit.BuildingModelExtractionOptions{
		IncludeWindows: true,
		AreaUnit:       "sqm",
	}

	if workset, ok := ctx.Options["workset"].(string); ok && workset != "" {
		options.Workset = workset
	}
	if wallType, ok := ctx.Options["wall-type"].(string); ok && wallType != "" {
		options.WallType = wallType
	}

	buildingModel, err := client.ExtractBuildingModel(options)
	if err != nil {
		return fmt.Errorf("failed to extract building model: %w", err)
	}

	walls, _ := buildingModel["walls"].([]interface{})
	windows, _ := buildingModel["windows"].([]interface{})

	fmt.Printf("Extracted %d walls and %d windows\n", len(walls), len(windows))

	output := "building-model.json"
	if out, ok := ctx.Options["output"].(string); ok && out != "" {
		output = out
	}

	if err := saveJSON(buildingModel, output); err != nil {
		return err
	}

	fmt.Printf("Building model saved to %s\n", output)
	fmt.Println("\nThis generic format can be analyzed by other tools or re-analyzed offline")
	return nil
}

func handleCalculateAreas(ctx *registry.TaskContext) error {
	fmt.Println("Calculating areas...")
	return nil
}

func handleFindClashes(ctx *registry.TaskContext) error {
	fmt.Println("Finding clashes...")
	return nil
}

func handleValidateStandards(ctx *registry.TaskContext) error {
	fmt.Println("Validating standards...")
	return nil
}

func handleUpdateParameters(ctx *registry.TaskContext) error {
	fmt.Println("Updating parameters...")
	return nil
}

func handleApplyTemplates(ctx *registry.TaskContext) error {
	fmt.Println("Applying templates...")
	return nil
}
