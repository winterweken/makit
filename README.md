# Makit

**A multi-tool CLI and TUI for AEC workflows** — orchestrate Revit, Rhino, Blender, IFC analysis, and MURB energy modelling from one interface.

Built in Rust with [rsille](https://github.com/nidhoggfgg/rsille) for braille-rendered terminal graphics and a native TUI framework. Makit bridges the gap between BIM authoring tools and programmatic analysis. Connect to live applications, extract building geometry, run cross-platform analytics, simulate energy performance, and visualize results — all from the terminal.

---

## Features

- **Unified CLI & Interactive TUI** — browse sources and actions in a tree-view explorer with real-time braille canvas preview
- **Live Revit Integration** — extract walls, floors, rooms, and run orientation/WWR analysis via a pyRevit HTTP bridge
- **IFC Analysis** — standalone wall orientation and Window-to-Wall Ratio analysis on IFC files (no Revit required)
- **Blender Sync Server** — receive live geometry from Blender over HTTP for visualization in the TUI
- **Rhino & Grasshopper** — import/export models and run Grasshopper definitions headlessly
- **MURB Energy Modelling** — monthly-timestep heat balance simulation for early-stage TEDI/TEUI/GHGI analysis via Python bridge
- **Braille Canvas Engine** — draw floor plans, elevations, energy charts, and data sheets using rsille's braille-character canvas
- **Animated 3D Logo** — rotating hexagonal logo rendered in braille on the TUI home screen
- **Cross-Platform Analysis** — extract from any source to a generic JSON format, then run the same analysis code everywhere

## Quick Start

### Install

```bash
git clone https://github.com/winterweken/makit.git
cd makit
cargo build --release
```

The binary is at `target/release/makit`.

### Launch the TUI

```bash
cargo run -p makit -- tui
```

Use `↑/↓` to navigate, `→/←` to expand/collapse, `Enter` to select, `Tab` to switch focus, `?` for help, and `Esc` to quit.

### CLI Commands

```bash
# List all registered tools, sources, and actions
makit list

# Execute a specific action
makit exec revit analysis revit-wall-orientations

# Analyze an IFC file
makit analyze examples/IFC/sample.ifc

# Show help
makit --help
```

### Canvas Demo

```bash
cargo run -p makit --example canvas_demo
```

Renders braille-character shapes, a floor plan with interior walls, and filled rectangles — straight in the terminal.

---

## Architecture

```
makit/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── makit-cli/           # Binary — clap CLI (list, exec, analyze, tui, status, init)
│   ├── makit-core/          # Registry singleton, config (figment/YAML), model types
│   ├── makit-geometry/      # Point, Line, Rectangle, Room, Floor + braille drawing
│   ├── makit-tools/         # Tool implementations (revit, rhino, blender, ifc, murb)
│   └── makit-tui/           # rsille-native TUI (tree explorer, canvas viz, theme)
├── pyrevit-extension/       # Python pyRevit extension (runs inside Revit)
├── scripts/
│   ├── blender/             # Python Blender addon
│   └── murb_runner.py       # Python bridge for MURB energy tool
├── examples/IFC/            # Sample IFC files
└── docs/                    # Architecture documentation
```

### Tool Registry

Tools register **sources** (geometry input drivers) and **actions** (operations) at startup:

| Source | Description |
|--------|-------------|
| `revit` | Autodesk Revit integration via pyRevit HTTP |
| `rhino` | Rhino 3D / Grasshopper |
| `blender` | Blender live geometry sync |
| `ifc` | IFC file loader |
| `murb` | MURB energy modelling tool |

| Action | Category | Description |
|--------|----------|-------------|
| `revit-extract-walls` | extraction | Extract wall elements from Revit |
| `revit-wall-orientations` | analysis | Analyze wall orientations + WWR |
| `murb-simulate` | analysis | Run monthly energy simulation |
| `murb-report` | reporting | Generate TEDI/TEUI/GHGI report |
| `architect-render-demo` | rendering | Render architectural demo |
| ... | ... | 12 actions total |

### Dependencies

- **rsille v3** — braille canvas, TUI widgets (tree, list, select, button, layout), terminal rendering
- **clap** — CLI argument parsing
- **figment** — config management (YAML + env vars)
- **reqwest** — HTTP client for pyRevit bridge
- **axum** — HTTP server for Blender sync
- **tokio** — async runtime

---

## Development

```bash
# Build
cargo build

# Run tests (47 tests across 5 crates)
cargo test

# Run the CLI
cargo run -p makit -- --help

# Run the TUI
cargo run -p makit -- tui

# Run the canvas demo
cargo run -p makit --example canvas_demo

# Format
cargo fmt

# Lint
cargo clippy
```

## License

MIT
