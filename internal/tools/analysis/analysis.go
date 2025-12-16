package analysis

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"github.com/winteweken/makit/internal/registry"
)

// RegisterTasks registers all analysis-related tasks
func RegisterTasks() {
	reg := registry.GetRegistry()
	tool := reg.RegisterTool("analysis", "Analysis and simulation tools")

	registerGeometricTasks(tool)
	registerPerformanceTasks(tool)
	registerIFCTasks(tool)
}

func registerGeometricTasks(tool *registry.Tool) {
	category := tool.AddCategory("geometric", "Geometric analysis and calculations")

	category.AddTask("volume-analysis", "Calculate volumes of geometric elements", func(ctx *registry.TaskContext) error {
		fmt.Println("Performing volume analysis...")
		return nil
	}).AddOption("unit", "Volume unit (cf, cm)", "string", false, "cf")

	category.AddTask("surface-area", "Calculate surface areas", func(ctx *registry.TaskContext) error {
		fmt.Println("Calculating surface areas...")
		return nil
	}).AddOption("unit", "Area unit (sqft, sqm)", "string", false, "sqft")

	category.AddTask("spatial-relationships", "Analyze spatial relationships between elements", func(ctx *registry.TaskContext) error {
		fmt.Println("Analyzing spatial relationships...")
		return nil
	})
}

func registerPerformanceTasks(tool *registry.Tool) {
	category := tool.AddCategory("performance", "Performance analysis and simulation")

	category.AddTask("energy-analysis", "Run energy performance analysis", func(ctx *registry.TaskContext) error {
		fmt.Println("Running energy analysis...")
		return nil
	}).AddOption("weather-file", "Path to weather data file", "string", true, nil)

	category.AddTask("daylighting", "Analyze daylighting performance", func(ctx *registry.TaskContext) error {
		fmt.Println("Analyzing daylighting...")
		return nil
	}).AddOption("grid-size", "Analysis grid size", "float", false, 1.0)

	category.AddTask("thermal-comfort", "Analyze thermal comfort", func(ctx *registry.TaskContext) error {
		fmt.Println("Analyzing thermal comfort...")
		return nil
	})
}

func registerIFCTasks(tool *registry.Tool) {
	category := tool.AddCategory("ifc", "IFC file analysis")

	category.AddTask("wall-orientation-wwr", "Analyze wall orientations and WWR from IFC files", func(ctx *registry.TaskContext) error {
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
