# Makit

A Go CLI tool for managing pyRevit extensions and automating Revit workflows.

## Features

- Manage pyRevit installations and extensions
- Run pyRevit scripts from the command line
- **Architectural Rendering Engine**: Create detailed TUI-based floor plans and elevations
- Configuration management for development workflows
- Cross-platform support (Windows, macOS, Linux)

## Quick Try: Architectural Demo

To immediately see the new architectural rendering capabilities (Solid Walls, WWR Analysis, Symbols):

```bash
# Clone the repository
git clone https://github.com/winteweken/makit.git
cd makit

# Run the demo
go run examples/canvas_demo.go
```

## Installation

```bash
go install github.com/winteweken/makit/cmd/makit@latest
```

Or build from source:

```bash
git clone https://github.com/winteweken/makit.git
cd makit
go build -o makit ./cmd/makit
```

## Architecture

Makit uses a hierarchical plugin system:

```text
Tool (e.g., Revit, Rhino, Analysis)
  └── Category (e.g., Geometry, Analysis)
      └── Task (e.g., Extract Walls, Calculate Areas)
```

This architecture allows you to:

- Easily add new tools and categories
- Organize tasks logically
- Execute tasks with a consistent interface
- Extend functionality without modifying core code

## Usage

### List Available Tools

```bash
makit list                    # Show all tools
makit list revit              # Show Revit categories
makit list revit geometry     # Show tasks in geometry category
```

### Execute Tasks

```bash
# Revit geometry extraction
makit exec revit geometry extract-walls --output walls.json
makit exec revit geometry extract-floors

# Revit analysis
makit exec revit analysis find-clashes --tolerance 0.1
makit exec revit analysis calculate-areas

# Rhino operations
makit exec rhino import-export import-revit --input model.rvt
makit exec rhino grasshopper run-definition --definition script.gh

# Analysis tools
makit exec analysis geometric volume-analysis --unit cf
makit exec analysis performance energy-analysis --weather-file data.epw
```

### Legacy Commands

```bash
makit init      # Initialize configuration
makit status    # Check pyRevit status
makit run       # Run pyRevit script
```

## Configuration

Edit `~/.makit.yaml` to customize settings:

```yaml
pyrevit:
  install_path: ""
  extensions_paths:
    - "~/pyRevit/extensions"
  default_revit_version: "2024"

general:
  editor: "code"
  auto_update: true
  log_level: "info"
```

## Development

### Project Structure

```text
makit/
├── cmd/
│   └── makit/              # Main application entry point
├── internal/
│   ├── cmd/                # CLI commands (list, exec, init, etc.)
│   ├── config/             # Configuration management
│   ├── pyrevit/            # pyRevit integration
│   ├── registry/           # Task registry system
│   │   ├── models.go       # Tool, Category, Task models
│   │   └── registry.go     # Global registry
│   └── tools/              # Tool implementations
│       ├── revit/          # Revit tool with categories
│       ├── rhino/          # Rhino tool with categories
│       └── analysis/       # Analysis tools
└── pkg/
    └── utils/              # Shared utilities
```

### Adding New Tools

To add a new tool:

1. Create a new package in `internal/tools/yourtool/`
2. Implement `RegisterTasks()` function:

```go
package yourtool

import "github.com/winteweken/makit/internal/registry"

func RegisterTasks() {
    reg := registry.GetRegistry()
    tool := reg.RegisterTool("yourtool", "Your tool description")

    category := tool.AddCategory("yourcategory", "Category description")

    category.AddTask("yourtask", "Task description", func(ctx *registry.TaskContext) error {
        // Your task implementation
        return nil
    }).AddOption("option-name", "Option description", "string", false, "default")
}
```

3. Register in `internal/cmd/root.go`:

```go
import "github.com/winteweken/makit/internal/tools/yourtool"

func registerTools() {
    revit.RegisterTasks()
    rhino.RegisterTasks()
    analysis.RegisterTasks()
    yourtool.RegisterTasks()  // Add your tool
}
```

### Building

```bash
go build -o makit ./cmd/makit
```

### Running Tests

```bash
go test ./...
```

## License

MIT
