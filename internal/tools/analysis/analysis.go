package analysis

import (
	"fmt"

	"github.com/winteweken/makit/internal/registry"
)

// RegisterTasks registers all analysis-related tasks
func RegisterTasks() {
	reg := registry.GetRegistry()
	tool := reg.RegisterTool("analysis", "Analysis and simulation tools")

	registerGeometricTasks(tool)
	registerPerformanceTasks(tool)
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
