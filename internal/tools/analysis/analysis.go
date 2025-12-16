package analysis

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"github.com/winteweken/makit/internal/registry"
)

// RegisterTasks registers all analysis-related tasks as Actions
func RegisterTasks() {

	reg := registry.GetRegistry()

	// Geometric Actions
	reg.RegisterAction("volume-analysis", "Calculate volumes of geometric elements", "geometric", func(ctx *registry.TaskContext) error {
		fmt.Println("Performing volume analysis on active geometry...")
		return nil
	}).AddOption("unit", "Volume unit (cf, cm)", "string", false, "cf")

	reg.RegisterAction("surface-area", "Calculate surface areas", "geometric", func(ctx *registry.TaskContext) error {
		fmt.Println("Calculating surface areas on active geometry...")
		return nil
	}).AddOption("unit", "Area unit (sqft, sqm)", "string", false, "sqft")

	// Performance Actions
	reg.RegisterAction("energy-analysis", "Run energy performance analysis", "performance", func(ctx *registry.TaskContext) error {
		fmt.Println("Running energy analysis on active geometry...")
		return nil
	}).AddOption("weather-file", "Path to weather data file", "string", true, nil)

	reg.RegisterAction("daylighting", "Analyze daylighting performance", "performance", func(ctx *registry.TaskContext) error {
		fmt.Println("Analyzing daylighting on active geometry...")
		return nil
	}).AddOption("grid-size", "Analysis grid size", "float", false, 1.0)

	// Register IFC Analysis as an Action (though it might handle its own source for now)
	registerIFCTasks(reg)
}

func registerIFCTasks(reg *registry.Registry) {
	// Keeping this one separate for now as it's complex
	reg.RegisterAction("wall-orientation-wwr", "Analyze wall orientations and WWR from IFC", "ifc", func(ctx *registry.TaskContext) error {
		// Get the IFC file path from options or use default example
		ifcFile := "examples/IFC/IFCSchependomlaan.ifc"
		if path, ok := ctx.Options["ifc-file"].(string); ok && path != "" {
			ifcFile = path
		}

		// Check if file exists
		if _, err := os.Stat(ifcFile); os.IsNotExist(err) {
			return fmt.Errorf("IFC file not found: %s", ifcFile)
		}

		customScript := ""
		if script, ok := ctx.Options["script"].(string); ok {
			customScript = script
		}

		scriptPath, err := ResolveAnalyzeScript(customScript)
		if err != nil {
			return err
		}

		// Convert IFC file to absolute path
		absIFCFile, err := filepath.Abs(ifcFile)
		if err != nil {
			return fmt.Errorf("failed to get absolute path for IFC file: %w", err)
		}

		// Get options
		areaUnit := "sqm"
		if unit, ok := ctx.Options["unit"].(string); ok && unit != "" {
			areaUnit = unit
		}

		// Build command arguments
		cmdArgs := []string{scriptPath, absIFCFile, "--unit", areaUnit}

		if output, ok := ctx.Options["output"].(string); ok && output != "" {
			cmdArgs = append(cmdArgs, "--output", output)
		}

		// Run the Python script
		pythonCmd := exec.Command("python3", cmdArgs...)
		pythonCmd.Stdout = os.Stdout
		pythonCmd.Stderr = os.Stderr

		fmt.Printf("Analyzing IFC file: %s\n", ifcFile)
		if err := pythonCmd.Run(); err != nil {
			return fmt.Errorf("analysis failed: %w", err)
		}

		return nil
	}).
		AddOption("ifc-file", "Path to IFC file", "string", false, "examples/IFC/IFCSchependomlaan.ifc").
		AddOption("unit", "Area unit (sqm or sqf)", "string", false, "sqm").
		AddOption("output", "Save results to JSON file", "string", false, "").
		AddOption("script", "Path to analyze_ifc.py script", "string", false, "")
}
