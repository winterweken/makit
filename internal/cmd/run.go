package cmd

import (
	"fmt"

	"github.com/winteweken/makit/internal/pyrevit"
	"github.com/spf13/cobra"
)

var runCmd = &cobra.Command{
	Use:   "run [script]",
	Short: "Run a pyRevit script",
	Long:  `Execute a pyRevit script in the context of Revit.`,
	Args:  cobra.MinimumNArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		scriptPath := args[0]

		fmt.Printf("Running script: %s\n", scriptPath)

		if err := pyrevit.RunScript(scriptPath); err != nil {
			return fmt.Errorf("failed to run script: %w", err)
		}

		fmt.Println("Script executed successfully")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(runCmd)
}
