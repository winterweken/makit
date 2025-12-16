package cmd

import (
	"fmt"
	"os"
	"os/exec"

	"github.com/spf13/cobra"
	"github.com/winteweken/makit/internal/tools/analysis"
)

var (
	outputFile   string
	areaUnit     string
	storeyFilter string
	wallType     string
	extractOnly  bool
	scriptPath   string
)

var analyzeCmd = &cobra.Command{
	Use:   "analyze [ifc-file]",
	Short: "Analyze IFC files for wall orientations and WWR",
	Long: `Analyze IFC files to calculate wall orientations and Window-to-Wall Ratios (WWR).

This command runs the IFC analysis tool on the specified file and generates
a detailed report showing wall orientations by compass direction and WWR calculations.

Example:
  makit analyze examples/IFC/Building-Architecture.ifc
  makit analyze model.ifc --output results.json --unit sqf
  makit analyze model.ifc --storey "Level 1"`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		ifcFile := args[0]

		// Check if file exists
		if _, err := os.Stat(ifcFile); os.IsNotExist(err) {
			return fmt.Errorf("IFC file not found: %s", ifcFile)
		}

		scriptPathAbs, err := analysis.ResolveAnalyzeScript(scriptPath)
		if err != nil {
			return err
		}

		// Build command arguments
		cmdArgs := []string{scriptPathAbs, ifcFile}

		if outputFile != "" {
			cmdArgs = append(cmdArgs, "--output", outputFile)
		}

		if areaUnit != "" {
			cmdArgs = append(cmdArgs, "--unit", areaUnit)
		}

		if storeyFilter != "" {
			cmdArgs = append(cmdArgs, "--storey", storeyFilter)
		}

		if wallType != "" {
			cmdArgs = append(cmdArgs, "--wall-type", wallType)
		}

		if extractOnly {
			cmdArgs = append(cmdArgs, "--extract-only")
		}

		// Run the Python script
		pythonCmd := exec.Command("python3", cmdArgs...)
		pythonCmd.Stdout = os.Stdout
		pythonCmd.Stderr = os.Stderr
		pythonCmd.Stdin = os.Stdin

		if err := pythonCmd.Run(); err != nil {
			return fmt.Errorf("analysis failed: %w", err)
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(analyzeCmd)

	analyzeCmd.Flags().StringVarP(&outputFile, "output", "o", "", "Save detailed JSON results to file")
	analyzeCmd.Flags().StringVar(&areaUnit, "unit", "sqm", "Area unit (sqm or sqf)")
	analyzeCmd.Flags().StringVar(&storeyFilter, "storey", "", "Filter by building storey name")
	analyzeCmd.Flags().StringVar(&wallType, "wall-type", "", "Filter by wall type")
	analyzeCmd.Flags().BoolVar(&extractOnly, "extract-only", false, "Only extract to generic format, skip analysis")
	analyzeCmd.Flags().StringVar(&scriptPath, "script", "", "Path to analyze_ifc.py script (overrides search path)")
}
