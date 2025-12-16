package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"github.com/winteweken/makit/internal/tools/analysis"
	"github.com/winteweken/makit/internal/tools/blender"
	"github.com/winteweken/makit/internal/tools/ifc"
	"github.com/winteweken/makit/internal/tools/revit"
	"github.com/winteweken/makit/internal/tools/rhino"
)

var cfgFile string

var rootCmd = &cobra.Command{
	Use:   "makit",
	Short: "A CLI tool for managing pyRevit extensions and workflows",
	Long: `Makit is a command-line interface tool that helps you manage
pyRevit extensions, automate Revit workflows, and integrate with
your development environment.`,
}

func Execute() error {
	return rootCmd.Execute()
}

func init() {
	cobra.OnInitialize(initConfig)
	cobra.OnInitialize(registerTools)

	rootCmd.PersistentFlags().StringVar(&cfgFile, "config", "", "config file (default is $HOME/.makit.yaml)")
	rootCmd.PersistentFlags().BoolP("verbose", "v", false, "verbose output")

	viper.BindPFlag("verbose", rootCmd.PersistentFlags().Lookup("verbose"))
}

func registerTools() {
	// Register all available tools
	revit.RegisterTasks()
	rhino.RegisterTasks()
	blender.RegisterTasks()
	analysis.RegisterTasks()
	ifc.RegisterTasks()
}

func initConfig() {
	if cfgFile != "" {
		viper.SetConfigFile(cfgFile)
	} else {
		home, err := os.UserHomeDir()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error getting home directory: %v\n", err)
			os.Exit(1)
		}

		viper.AddConfigPath(home)
		viper.SetConfigType("yaml")
		viper.SetConfigName(".makit")
	}

	viper.AutomaticEnv()

	if err := viper.ReadInConfig(); err == nil {
		if viper.GetBool("verbose") {
			fmt.Fprintln(os.Stderr, "Using config file:", viper.ConfigFileUsed())
		}
	}
}
