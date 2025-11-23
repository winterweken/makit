package cmd

import (
	"fmt"

	"github.com/spf13/cobra"
	"github.com/winteweken/makit/internal/registry"
)

var execCmd = &cobra.Command{
	Use:   "exec <tool> <category> <task>",
	Short: "Execute a specific task",
	Long: `Execute a task from a tool's category.

Examples:
  makit exec revit geometry extract-walls --output walls.json
  makit exec revit analysis find-clashes --tolerance 0.1
  makit exec analysis geometric volume-analysis --unit cf`,
	Args: cobra.ExactArgs(3),
	RunE: func(cmd *cobra.Command, args []string) error {
		toolName := args[0]
		categoryName := args[1]
		taskName := args[2]

		reg := registry.GetRegistry()

		fmt.Printf("Executing: %s > %s > %s\n", toolName, categoryName, taskName)
		fmt.Println()

		ctx := &registry.TaskContext{
			Tool:     toolName,
			Category: categoryName,
			Task:     taskName,
			Options:  make(map[string]interface{}),
			Args:     cmd.Flags().Args(),
		}

		if err := reg.ExecuteTask(toolName, categoryName, taskName, ctx); err != nil {
			return fmt.Errorf("execution failed: %w", err)
		}

		fmt.Println()
		fmt.Println("Task completed successfully")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(execCmd)
}
