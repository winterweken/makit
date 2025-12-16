package cmd

import (
	"fmt"
	"strconv"
	"strings"

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
		task, err := reg.GetTask(toolName, categoryName, taskName)
		if err != nil {
			return err
		}

		fmt.Printf("Executing: %s > %s > %s\n", toolName, categoryName, taskName)
		fmt.Println()

		options, err := buildTaskOptions(task, cmd)
		if err != nil {
			return err
		}

		ctx := &registry.TaskContext{
			Tool:     toolName,
			Category: categoryName,
			Task:     taskName,
			Options:  options,
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

	execCmd.Flags().StringArray("set", []string{}, "Set task options as key=value (repeatable)")
}

func buildTaskOptions(task *registry.Task, cmd *cobra.Command) (map[string]interface{}, error) {
	options := make(map[string]interface{})
	optDefs := make(map[string]registry.TaskOption)

	for _, opt := range task.Options {
		optDefs[opt.Name] = opt
		if opt.Default != nil {
			options[opt.Name] = opt.Default
		}
	}

	rawSets, err := cmd.Flags().GetStringArray("set")
	if err != nil {
		return nil, err
	}

	for _, raw := range rawSets {
		parts := strings.SplitN(raw, "=", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("invalid option format %q, expected key=value", raw)
		}

		key := strings.TrimSpace(parts[0])
		value := strings.TrimSpace(parts[1])

		opt, ok := optDefs[key]
		if !ok {
			return nil, fmt.Errorf("unknown option %q for task %s", key, task.Name)
		}

		parsed, err := parseOptionValue(opt, value)
		if err != nil {
			return nil, fmt.Errorf("invalid value for --set %s: %w", key, err)
		}

		options[key] = parsed
	}

	for name, opt := range optDefs {
		_, present := options[name]
		if opt.Required && !present {
			return nil, fmt.Errorf("missing required option %q", name)
		}
	}

	return options, nil
}

func parseOptionValue(opt registry.TaskOption, value string) (interface{}, error) {
	switch strings.ToLower(opt.Type) {
	case "bool", "boolean":
		return strconv.ParseBool(value)
	case "int", "integer":
		return strconv.Atoi(value)
	case "float", "float64", "number":
		return strconv.ParseFloat(value, 64)
	default:
		return value, nil
	}
}
