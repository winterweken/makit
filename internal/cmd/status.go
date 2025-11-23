package cmd

import (
	"fmt"

	"github.com/winteweken/makit/internal/pyrevit"
	"github.com/spf13/cobra"
)

var statusCmd = &cobra.Command{
	Use:   "status",
	Short: "Check pyRevit installation status",
	Long:  `Display information about the current pyRevit installation and configuration.`,
	RunE: func(cmd *cobra.Command, args []string) error {
		status, err := pyrevit.GetStatus()
		if err != nil {
			return fmt.Errorf("failed to get pyRevit status: %w", err)
		}

		fmt.Println("pyRevit Status:")
		fmt.Printf("  Installed: %v\n", status.Installed)
		if status.Installed {
			fmt.Printf("  Version: %s\n", status.Version)
			fmt.Printf("  Path: %s\n", status.InstallPath)
			fmt.Printf("  Extensions: %d\n", len(status.Extensions))
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(statusCmd)
}
