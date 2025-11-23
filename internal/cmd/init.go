package cmd

import (
	"fmt"

	"github.com/winteweken/makit/internal/config"
	"github.com/spf13/cobra"
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Initialize makit configuration",
	Long:  `Initialize makit configuration and set up the environment for pyRevit integration.`,
	RunE: func(cmd *cobra.Command, args []string) error {
		fmt.Println("Initializing makit configuration...")

		if err := config.Initialize(); err != nil {
			return fmt.Errorf("failed to initialize config: %w", err)
		}

		fmt.Println("Configuration initialized successfully!")
		fmt.Println("Edit ~/.makit.yaml to customize your settings.")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(initCmd)
}
