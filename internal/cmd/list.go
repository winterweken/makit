package cmd

import (
	"fmt"
	"strings"

	"github.com/spf13/cobra"
	"github.com/winteweken/makit/internal/registry"
)

var listCmd = &cobra.Command{
	Use:   "list [tool] [category]",
	Short: "List available tools, categories, and tasks",
	Long: `Display the hierarchical structure of tools, categories, and tasks.

Examples:
  makit list                    # List all tools
  makit list revit              # List categories in Revit tool
  makit list revit geometry     # List tasks in Revit geometry category`,
	Args: cobra.MaximumNArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		reg := registry.GetRegistry()

		if len(args) == 0 {
			return listTools(reg)
		}

		if len(args) == 1 {
			return listCategories(reg, args[0])
		}

		return listTasks(reg, args[0], args[1])
	},
}

func listTools(reg *registry.Registry) error {
	tools := reg.ListTools()

	if len(tools) == 0 {
		fmt.Println("No tools registered")
		return nil
	}

	fmt.Println("Available Tools:")
	fmt.Println(strings.Repeat("-", 50))

	for _, tool := range tools {
		fmt.Printf("  %s - %s\n", tool.Name, tool.Description)
		fmt.Printf("    Categories: %d\n\n", len(tool.Categories))
	}

	fmt.Println("Use 'makit list <tool>' to see categories")
	return nil
}

func listCategories(reg *registry.Registry, toolName string) error {
	tool, err := reg.GetTool(toolName)
	if err != nil {
		return err
	}

	fmt.Printf("Tool: %s\n", tool.Name)
	fmt.Println(strings.Repeat("-", 50))

	if len(tool.Categories) == 0 {
		fmt.Println("No categories available")
		return nil
	}

	for _, category := range tool.Categories {
		fmt.Printf("\n  Category: %s\n", category.Name)
		fmt.Printf("  Description: %s\n", category.Description)
		fmt.Printf("  Tasks: %d\n", len(category.Tasks))
	}

	fmt.Printf("\nUse 'makit list %s <category>' to see tasks\n", toolName)
	return nil
}

func listTasks(reg *registry.Registry, toolName, categoryName string) error {
	tool, err := reg.GetTool(toolName)
	if err != nil {
		return err
	}

	category, err := tool.GetCategory(categoryName)
	if err != nil {
		return err
	}

	fmt.Printf("Tool: %s > Category: %s\n", tool.Name, category.Name)
	fmt.Println(strings.Repeat("-", 50))

	if len(category.Tasks) == 0 {
		fmt.Println("No tasks available")
		return nil
	}

	for _, task := range category.Tasks {
		fmt.Printf("\n  Task: %s\n", task.Name)
		fmt.Printf("  Description: %s\n", task.Description)

		if len(task.Options) > 0 {
			fmt.Println("  Options:")
			for _, opt := range task.Options {
				req := ""
				if opt.Required {
					req = " (required)"
				}
				fmt.Printf("    --%s: %s%s\n", opt.Name, opt.Description, req)
			}
		}
	}

	fmt.Printf("\nUse 'makit exec %s %s <task>' to execute a task\n", toolName, categoryName)
	return nil
}

func init() {
	rootCmd.AddCommand(listCmd)
}
