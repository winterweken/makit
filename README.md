# Makit

**A multi-tool CLI and TUI for AEC workflows** — orchestrate Revit, Rhino, Blender, and IFC analysis from one interface.

Built in Go with a plugin-based architecture, Makit bridges the gap between BIM authoring tools and programmatic analysis. Connect to live applications, extract building geometry, run cross-platform analytics, and visualize results — all from the terminal.

---

## Features

- **Unified CLI & Interactive TUI** — browse sources and actions in a tree-view explorer with real-time geometry preview
- **Live Revit Integration** — extract walls, floors, rooms, and run orientation/WWR analysis via a pyRevit HTTP bridge
- **IFC Analysis** — standalone wall orientation and Window-to-Wall Ratio analysis on IFC files (no Revit required)
- **Blender Sync Server** — receive live geometry from Blender over HTTP for visualization in the TUI
- **Rhino & Grasshopper** — import/export models and run Grasshopper definitions headlessly
- **Architectural Rendering Engine** — draw floor plans, elevations, and data sheets using a braille-character canvas
- **3D SDF Logo** — interactive rotating logo rendered via the `sdfx` geometry kernel on the TUI home screen
- **Cross-Platform Analysis** — extract from any source to a generic JSON format, then run the same analysis code everywhere

## Quick Start

### Install

```bash
go install github.com/winteweken/makit/cmd/makit@latest
```

Or build from source:

```bash
git clone https://github.com/winteweken/makit.git
cd makit
go build -o makit ./cmd/makit
```

### Launch the TUI

```bash
makit tui
```

Use `↑/↓` to navigate, `Enter` to expand/select, `Tab` to switch panes, `x` to execute, and `q` to quit.

### Try the Architectural Demo

```bash
go run examples/canvas_demo.go
```

Renders a braille-character floor plan, south elevation with WWR analysis, and project data — straight in the terminal.

---

## Architecture

Makit uses a **Source → Action** model backed by a plugin registry:

```
┌──────────────────────────────────────────────────────────┐
│                     CLI / TUI Layer                      │
│  makit list · makit exec · makit analyze · makit tui     │
└────────────────────────┬─────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────┐
│                   Registry (Go)                          │
│  Sources: revit · rhino · blender · ifc                  │
│  Actions: extract-walls · wall-orientations · wwr · ...  │
└────────────────────────┬─────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────┐
│              Generic Format (JSON)                       │
│  Platform-agnostic walls, windows, rooms, metadata       │
└────────────────────────┬─────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────┐
│          Platform-Specific Extractors                    │
│  Revit (pyRevit HTTP) · IFC (ifcopenshell) · Blender     │
└──────────────────────────────────────────────────────────┘
```

**Sources** are geometry input drivers (Revit, Rhino, Blender, IFC files).
**Actions** are operations performed on geometry (extract, analyze, export).

This separation means the same analysis code works regardless of which tool produced the geometry.

---

## CLI Reference

### `makit list`

Browse the tool/category/task hierarchy:

```bash
makit list                        # Show all registered tools
makit list revit                  # Show Revit categories
makit list revit geometry         # Show tasks in a category
```

### `makit exec`

Execute a specific task with options:

```bash
# Revit extraction
makit exec revit geometry extract-walls --set output=walls.json
makit exec revit geometry extract-floors
makit exec revit geometry extract-rooms

# Revit analysis
makit exec revit analysis wall-orientations --set workset=ENVELOPE --set unit=sqm
makit exec revit analysis find-clashes --set tolerance=0.1
makit exec revit analysis calculate-areas

# Rhino
makit exec rhino import-export import-revit --set input=model.rvt
makit exec rhino grasshopper run-definition --set definition=script.gh

# Architect rendering
makit exec architect render demo
```

### `makit analyze`

Direct IFC analysis shortcut:

```bash
makit analyze model.ifc
makit analyze model.ifc --output results.json --unit sqf
makit analyze model.ifc --storey "Level 1" --wall-type Exterior
makit analyze model.ifc --extract-only    # Generic JSON only, skip analysis
```

### `makit tui`

Launch the interactive terminal UI with a tree explorer, geometry visualization, and isometric face rendering.

### `makit init` / `makit status` / `makit run`

Legacy pyRevit workflow commands:

```bash
makit init      # Initialize configuration
makit status    # Check pyRevit connection status
makit run       # Run a pyRevit script
```

---

## Configuration

Edit `~/.makit.yaml`:

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

---

## Registered Tools

| Tool | Type | Description |
|------|------|-------------|
| **revit** | Source + Actions | Extract walls/floors/rooms, wall orientation analysis, clash detection, standards validation, parameter updates |
| **rhino** | Source + Actions | Import/export models, run Grasshopper definitions, bake geometry |
| **blender** | Source | Live geometry sync server (HTTP on port 8085) |
| **ifc** | Source | Load IFC files for analysis |
| **architect** | Actions | TUI-based architectural rendering demo |
| **analysis** | Actions | Volume, surface area, energy, daylighting, and IFC wall-orientation/WWR analysis |

---

## pyRevit Extension

The `pyrevit-extension/` directory contains a packaged pyRevit extension that runs an HTTP server inside Revit, exposing geometry extraction and analysis endpoints.

**Setup:** Copy `Makit.extension` to your pyRevit extensions folder and reload.

**API Endpoints:**
- `GET /health` — server health check
- `GET /api/project/info` — project metadata
- `POST /api/geometry/walls` — extract wall elements
- `POST /api/geometry/floors` — extract floor elements
- `POST /api/geometry/rooms` — extract room elements
- `POST /api/analysis/wall-orientations` — wall orientation + WWR analysis
- `POST /api/extraction/building-model` — extract to generic format

See [`pyrevit-extension/README.md`](pyrevit-extension/README.md) for full installation and development instructions.

---

## Project Structure

```
makit/
├── cmd/makit/                  # CLI entrypoint
├── internal/
│   ├── cmd/                    # Cobra commands (list, exec, analyze, tui, init, status, run)
│   ├── config/                 # Configuration management
│   ├── pyrevit/                # pyRevit HTTP client and data models
│   ├── registry/               # Source/Action/Task registry core
│   │   ├── models.go           # Tool, Category, Task, Source, Action types
│   │   └── registry.go         # Global singleton registry
│   ├── tools/                  # Tool implementations (registered via RegisterTasks())
│   │   ├── revit/              # Revit source + extraction/analysis actions
│   │   ├── rhino/              # Rhino source + import/export/Grasshopper actions
│   │   ├── blender/            # Blender sync server (HTTP geometry receiver)
│   │   ├── ifc/                # IFC file source
│   │   ├── architect/          # Architectural rendering demo
│   │   └── analysis/           # Standalone analysis actions (geometric, performance, IFC)
│   └── tui/                    # Bubble Tea TUI (tree explorer, viz, options, themes)
├── pkg/
│   ├── canvas/                 # Braille-character 2D drawing surface with ANSI color
│   └── geometry/               # Point/Line/Rectangle types, wall drawing, SDF renderer
├── pyrevit-extension/          # Packaged pyRevit extension (Python)
├── scripts/blender/            # Blender connector script
├── examples/
│   ├── canvas_demo.go          # Standalone architectural rendering demo
│   └── IFC/                    # Sample IFC files and analysis scripts
├── docs/                       # Architecture and analysis documentation
└── go.mod                      # Go 1.25+, Cobra, Viper, Bubble Tea, sdfx
```

---

## Development

### Prerequisites

- **Go 1.25+**
- **Python 3** (for IFC analysis and pyRevit integration)
- **ifcopenshell** (Python, only for IFC analysis)

### Build & Test

```bash
go build -o makit ./cmd/makit    # Build binary
go test ./...                    # Run test suite
go vet ./...                     # Static analysis
go fmt ./...                     # Format code
```

### Adding a New Tool

1. Create a package in `internal/tools/yourtool/`
2. Implement `RegisterTasks()` to register sources and/or actions:

```go
package yourtool

import "github.com/winteweken/makit/internal/registry"

func RegisterTasks() {
    reg := registry.GetRegistry()

    // Register as a geometry source
    reg.RegisterSource("yourtool", "Description", func(ctx *registry.TaskContext) error {
        // Connect to your tool
        return nil
    })

    // Register actions
    reg.RegisterAction("yourtool-analyze", "Run analysis", "analysis", func(ctx *registry.TaskContext) error {
        // Your analysis logic
        return nil
    }).AddOption("output", "Output file", "string", false, "result.json")
}
```

3. Wire it up in `internal/cmd/root.go`:

```go
import "github.com/winteweken/makit/internal/tools/yourtool"

func registerTools() {
    // ...existing tools...
    yourtool.RegisterTasks()
}
```

---

## Documentation

- [Hybrid Architecture Overview](docs/HYBRID_ARCHITECTURE.md) — 3-layer extraction/analysis design
- [Wall Orientation Analysis](docs/WALL_ORIENTATION_ANALYSIS.md) — compass classification and WWR
- [IFC Support](docs/IFC_SUPPORT.md) — standalone IFC analysis guide
- [pyRevit Extension](pyrevit-extension/README.md) — HTTP server setup and API reference
- [IFC Examples](examples/IFC/README.md) — sample files and analysis scripts

## License

[MIT](LICENSE) © winterweken
