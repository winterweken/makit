package cmd

import (
	"fmt"
	"os"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/spf13/cobra"
	"github.com/winteweken/makit/internal/tui"
)

var tuiCmd = &cobra.Command{
	Use:   "tui",
	Short: "Launch interactive TUI interface",
	Long: `Launch the terminal user interface for interactive navigation
and execution of tasks with geometry preview.`,
	RunE: func(cmd *cobra.Command, args []string) error {
		p := tea.NewProgram(
			tui.NewModel(),
			tea.WithAltScreen(),
		)

		if _, err := p.Run(); err != nil {
			fmt.Fprintf(os.Stderr, "Error running TUI: %v\n", err)
			return err
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(tuiCmd)
}
