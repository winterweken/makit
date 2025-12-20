package registry

import (
	"fmt"
	"sort"
	"sync"
)

// Registry holds all registered tools, categories, tasks, sources, and actions
type Registry struct {
	mu      sync.RWMutex
	Tools   map[string]*Tool
	Sources map[string]*Source
	Actions map[string]*Action
}

var (
	globalRegistry *Registry
	once           sync.Once
)

// GetRegistry returns the global registry instance
func GetRegistry() *Registry {
	once.Do(func() {
		globalRegistry = &Registry{
			Tools:   make(map[string]*Tool),
			Sources: make(map[string]*Source),
			Actions: make(map[string]*Action),
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

// RegisterSource registers a new geometry source
func (r *Registry) RegisterSource(name, description string, handler TaskHandler) *Source {
	r.mu.Lock()
	defer r.mu.Unlock()

	source := &Source{
		Name:        name,
		Description: description,
		Handler:     handler,
		Options:     []TaskOption{},
	}
	r.Sources[name] = source
	return source
}

// RegisterAction registers a new action
func (r *Registry) RegisterAction(name, description, category string, handler TaskHandler) *Action {
	r.mu.Lock()
	defer r.mu.Unlock()

	action := &Action{
		Name:        name,
		Description: description,
		Category:    category,
		Handler:     handler,
		Options:     []TaskOption{},
	}
	r.Actions[name] = action
	return action
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

// ListTools returns all registered tools sorted by name
func (r *Registry) ListTools() []*Tool {
	r.mu.RLock()
	defer r.mu.RUnlock()

	tools := make([]*Tool, 0, len(r.Tools))
	for _, tool := range r.Tools {
		tools = append(tools, tool)
	}

	sort.Slice(tools, func(i, j int) bool {
		return tools[i].Name < tools[j].Name
	})

	return tools
}

// ListSources returns all registered sources sorted by name
func (r *Registry) ListSources() []*Source {
	r.mu.RLock()
	defer r.mu.RUnlock()

	sources := make([]*Source, 0, len(r.Sources))
	for _, source := range r.Sources {
		sources = append(sources, source)
	}

	sort.Slice(sources, func(i, j int) bool {
		return sources[i].Name < sources[j].Name
	})

	return sources
}

// ListActions returns all registered actions sorted by name
func (r *Registry) ListActions() []*Action {
	r.mu.RLock()
	defer r.mu.RUnlock()

	actions := make([]*Action, 0, len(r.Actions))
	for _, action := range r.Actions {
		actions = append(actions, action)
	}

	sort.Slice(actions, func(i, j int) bool {
		return actions[i].Name < actions[j].Name
	})

	return actions
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
