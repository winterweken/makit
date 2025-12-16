package registry

import (
	"fmt"
)

// Tool represents a top-level tool (e.g., Revit, Rhino, Analysis)
type Tool struct {
	Name        string
	Description string
	Categories  map[string]*Category
}

// Category represents a category of tasks within a tool
type Category struct {
	Name        string
	Description string
	Tool        *Tool
	Tasks       map[string]*Task
}

// Task represents a specific task that can be executed
type Task struct {
	Name        string
	Description string
	Category    *Category
	Handler     TaskHandler
	Options     []TaskOption
}

// TaskHandler is a function that executes a task
type TaskHandler func(ctx *TaskContext) error

// TaskContext holds the execution context for a task
type TaskContext struct {
	Tool     string
	Category string
	Task     string
	Options  map[string]interface{}
	Args     []string
}

// TaskOption represents a configurable option for a task
type TaskOption struct {
	Name        string
	Description string
	Type        string
	Required    bool
	Default     interface{}
}

// NewTool creates a new tool
func NewTool(name, description string) *Tool {
	return &Tool{
		Name:        name,
		Description: description,
		Categories:  make(map[string]*Category),
	}
}

// AddCategory adds a category to a tool
func (t *Tool) AddCategory(name, description string) *Category {
	category := &Category{
		Name:        name,
		Description: description,
		Tool:        t,
		Tasks:       make(map[string]*Task),
	}
	t.Categories[name] = category
	return category
}

// GetCategory retrieves a category by name
func (t *Tool) GetCategory(name string) (*Category, error) {
	category, exists := t.Categories[name]
	if !exists {
		return nil, fmt.Errorf("category '%s' not found in tool '%s'", name, t.Name)
	}
	return category, nil
}

// AddTask adds a task to a category
func (c *Category) AddTask(name, description string, handler TaskHandler) *Task {
	task := &Task{
		Name:        name,
		Description: description,
		Category:    c,
		Handler:     handler,
		Options:     []TaskOption{},
	}
	c.Tasks[name] = task
	return task
}

// GetTask retrieves a task by name
func (c *Category) GetTask(name string) (*Task, error) {
	task, exists := c.Tasks[name]
	if !exists {
		return nil, fmt.Errorf("task '%s' not found in category '%s'", name, c.Name)
	}
	return task, nil
}

// AddOption adds an option to a task
func (t *Task) AddOption(name, description, optType string, required bool, defaultValue interface{}) *Task {
	option := TaskOption{
		Name:        name,
		Description: description,
		Type:        optType,
		Required:    required,
		Default:     defaultValue,
	}
	t.Options = append(t.Options, option)
	return t
}

// Execute runs the task handler
func (t *Task) Execute(ctx *TaskContext) error {
	if t.Handler == nil {
		return fmt.Errorf("no handler defined for task '%s'", t.Name)
	}
	return t.Handler(ctx)
}

// Source represents a geometry input driver (e.g. Blender, Revit, IFC)
type Source struct {
	Name        string
	Description string
	Handler     TaskHandler
	Options     []TaskOption
}

// Action represents an operation performed on the geometry (e.g. Analysis, Export)
type Action struct {
	Name        string
	Description string
	Category    string
	Handler     TaskHandler
	Options     []TaskOption
}

// AddOption adds an option to a Source
func (s *Source) AddOption(name, description, optType string, required bool, defaultValue interface{}) *Source {
	option := TaskOption{
		Name:        name,
		Description: description,
		Type:        optType,
		Required:    required,
		Default:     defaultValue,
	}
	s.Options = append(s.Options, option)
	return s
}

// AddOption adds an option to an Action
func (a *Action) AddOption(name, description, optType string, required bool, defaultValue interface{}) *Action {
	option := TaskOption{
		Name:        name,
		Description: description,
		Type:        optType,
		Required:    required,
		Default:     defaultValue,
	}
	a.Options = append(a.Options, option)
	return a
}
