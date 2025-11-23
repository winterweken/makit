package registry

import (
	"fmt"
	"sync"
)

// Registry holds all registered tools, categories, and tasks
type Registry struct {
	mu    sync.RWMutex
	Tools map[string]*Tool
}

var (
	globalRegistry *Registry
	once           sync.Once
)

// GetRegistry returns the global registry instance
func GetRegistry() *Registry {
	once.Do(func() {
		globalRegistry = &Registry{
			Tools: make(map[string]*Tool),
		}
	})
	return globalRegistry
}

// RegisterTool registers a new tool
func (r *Registry) RegisterTool(name, description string) *Tool {
	r.mu.Lock()
	defer r.mu.Unlock()

	tool := NewTool(name, description)
	r.Tools[name] = tool
	return tool
}

// GetTool retrieves a tool by name
func (r *Registry) GetTool(name string) (*Tool, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()

	tool, exists := r.Tools[name]
	if !exists {
		return nil, fmt.Errorf("tool '%s' not found", name)
	}
	return tool, nil
}

// ListTools returns all registered tools
func (r *Registry) ListTools() []*Tool {
	r.mu.RLock()
	defer r.mu.RUnlock()

	tools := make([]*Tool, 0, len(r.Tools))
	for _, tool := range r.Tools {
		tools = append(tools, tool)
	}
	return tools
}

// GetTask retrieves a task by tool, category, and task name
func (r *Registry) GetTask(toolName, categoryName, taskName string) (*Task, error) {
	tool, err := r.GetTool(toolName)
	if err != nil {
		return nil, err
	}

	category, err := tool.GetCategory(categoryName)
	if err != nil {
		return nil, err
	}

	task, err := category.GetTask(taskName)
	if err != nil {
		return nil, err
	}

	return task, nil
}

// ExecuteTask executes a task with the given context
func (r *Registry) ExecuteTask(toolName, categoryName, taskName string, ctx *TaskContext) error {
	task, err := r.GetTask(toolName, categoryName, taskName)
	if err != nil {
		return err
	}

	ctx.Tool = toolName
	ctx.Category = categoryName
	ctx.Task = taskName

	return task.Execute(ctx)
}
