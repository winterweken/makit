package registry

import (
	"testing"
)

func TestGetRegistry(t *testing.T) {
	reg1 := GetRegistry()
	reg2 := GetRegistry()

	if reg1 != reg2 {
		t.Error("GetRegistry() did not return the same instance")
	}
}

func TestRegistry_RegisterTool(t *testing.T) {
	// Reset registry for testing (hack for singleton)
	globalRegistry = &Registry{
		Tools: make(map[string]*Tool),
	}
	r := GetRegistry()

	tool := r.RegisterTool("test-tool", "Test Description")

	if tool.Name != "test-tool" {
		t.Errorf("got tool name %q, want %q", tool.Name, "test-tool")
	}

	got, err := r.GetTool("test-tool")
	if err != nil {
		t.Errorf("GetTool() error = %v", err)
	}
	if got != tool {
		t.Error("GetTool() returned different instance")
	}
}

func TestRegistry_Hierarchy(t *testing.T) {
	// Reset registry
	globalRegistry = &Registry{
		Tools: make(map[string]*Tool),
	}
	r := GetRegistry()

	// 1. Register Tool
	tool := r.RegisterTool("build", "Build stuff")

	// 2. Add Category
	cat := tool.AddCategory("compile", "Compilers")

	// 3. Add Task
	taskExecuted := false
	task := cat.AddTask("go", "Go Compiler", func(ctx *TaskContext) error {
		taskExecuted = true
		return nil
	})

	// Verify linkage
	if cat.Tool != tool {
		t.Error("Category.Tool link incorrect")
	}
	if task.Category != cat {
		t.Error("Task.Category link incorrect")
	}

	// Verify retrieval
	gotTask, err := r.GetTask("build", "compile", "go")
	if err != nil {
		t.Errorf("GetTask() error = %v", err)
	}
	if gotTask != task {
		t.Error("GetTask() returned different task")
	}

	// Verify execution
	ctx := &TaskContext{
		Tool:     "build",
		Category: "compile",
		Task:     "go",
	}
	err = r.ExecuteTask("build", "compile", "go", ctx)
	if err != nil {
		t.Errorf("ExecuteTask() error = %v", err)
	}
	if !taskExecuted {
		t.Error("Task handler was not executed")
	}
}

func TestTask_Options(t *testing.T) {
	task := &Task{
		Name:    "test",
		Options: []TaskOption{},
	}

	task.AddOption("force", "Force execution", "bool", false, false)

	if len(task.Options) != 1 {
		t.Errorf("got %d options, want 1", len(task.Options))
	}

	opt := task.Options[0]
	if opt.Name != "force" {
		t.Errorf("got option name %q, want %q", opt.Name, "force")
	}
}
