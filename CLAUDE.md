# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**makit** is a Rust workspace — a multi-tool CLI & TUI for AEC (architecture/engineering/construction) workflows. It bridges BIM authoring tools (Revit, Blender, Rhino) and analysis (IFC, MURB energy modelling) from one terminal, rendering geometry as braille-character graphics via [rsille](https://github.com/nidhoggfgg/rsille).

> **Note:** `cmd/`, `internal/`, and `pkg/` are vestigial Go scaffolding (`cmd/makit/main.go` has a dangling import to a nonexistent module; `internal/` and `pkg/` are empty). The real project is the Rust workspace under `crates/`. Ignore the Go files.

`AGENTS.md` is the canonical contributor guide and is more detailed than this file — read it for module-by-module breakdowns, coding style, and the "Before Modifying Code" trap table. `docs/` holds deep-dives loaded on demand (see triggers below).

## Commands

```bash
cargo build                              # build the workspace
cargo run -p makit -- tui                # launch the TUI (also: cargo run -p makit)
cargo run -p makit -- list               # list registered tools/sources/actions
cargo run -p makit -- exec revit analysis revit-wall-orientations
cargo run -p makit -- analyze examples/IFC/Building-Architecture.ifc   # WWR + envelope; extracts first if needed
cargo run -p makit -- extract examples/IFC/Building-Architecture.ifc   # needs a Python with IfcOpenShell
cargo run -p makit --example canvas_demo # braille canvas demo
cargo test                               # full suite (tests live as #[cfg(test)] mod tests in each module)
cargo test -p makit-geometry             # single crate
cargo test wall_orientation              # single test by name substring
cargo fmt && cargo clippy                # required clean before any PR
```

The binary package is named `makit` (in `crates/makit-cli`), so `-p makit` targets the CLI.

## Architecture

Five crates, layered:

- **`makit-core`** — the **Registry singleton** (`registry.rs`) is the heart: tools call `register_tasks()` at startup to register **sources** (geometry input drivers: `revit`, `blender`, `ifc`, `murb`, `rhino`) and **actions** (operations like `revit-wall-orientations`, `murb-simulate`). The CLI and TUI both dispatch through this registry. Also holds config (`figment`/YAML) and model types (`Tool`, `Category`, `Task`, `Source`, `Action`).
- **`makit-geometry`** — pure primitives (`Point`, `Line`, `Rectangle`, `Room`, `Floor`), braille drawing (scanline fill, wall rendering, isometric projection), and an SDF engine (`sdf.rs`). Keep this crate pure and testable.
- **`makit-tools`** — one file per integration, each registering tasks into the registry. The **hybrid 3-layer bridge pattern** (see `docs/HYBRID_ARCHITECTURE.md`) is central: every source extracts into one **generic JSON schema**, so the same analysis runs regardless of origin (live Revit, IFC file, etc.).
- **`makit-tui`** — rsille-native TUI following the **Elm architecture**: `State` struct, `Msg` enum, pure `update(state, msg)` reducer, pure `view(state) -> Widget`. Canvas preview routing keys off the `active_node.contains(...)` pattern in `app.rs`.
- **`makit-cli`** — thin clap orchestration in `src/commands/` (`list`, `exec`, `analyze`, `extract`, `tui`). Keep logic in the libraries; commands stay thin.

### External process bridges (non-obvious)

These tools shell out to / talk to external runtimes — they won't work without the counterpart running:

- **Revit** — async `reqwest` HTTP client to the pyRevit bridge on **port 48884** (`crates/makit-tools/src/revit/`, extension in `pyrevit-extension/`).
- **Blender** — `axum` HTTP server with `Arc<RwLock>` shared state receiving live geometry (`blender.rs`, addon in `scripts/blender/`).
- **MURB energy** — Python subprocess over JSON IPC; needs the `murb_energy_tool` Python package (`murb.rs` ↔ `scripts/murb_runner.py`). `murb-simulate --model` fills GFA, wall, window and roof areas from an IFC extraction (`building_model::envelope`); typed values win.
- **IFC extraction** — makit runs `pyrevit-extension/Makit.extension/lib/analyze_ifc.py` (IfcOpenShell) as a subprocess when an IFC's JSON beside it is missing or stale (`ifc.rs`); the Python comes from `python.rs` (`MAKIT_PYTHON`, the repository's `.venv`, `python3`). A fresh JSON never needs Python.

### Error handling convention

`anyhow::Result` for CLI/tool orchestration; `thiserror` typed errors for library-level code.

## docs/ trigger index

Read the deep-dive before touching the corresponding area:

| Working on | Read |
|---|---|
| Revit bridge, wall orientation, WWR, compass bucketing (`atan2` axis convention) | `docs/WALL_ORIENTATION_ANALYSIS.md` |
| IFC parsing/analysis (IfcOpenShell, storey filtering, units) | `docs/IFC_SUPPORT.md` |
| Bridge architecture / generic JSON schema (3-layer separation) | `docs/HYBRID_ARCHITECTURE.md` |

## Dependency note

`rsille` is pulled from git (`branch = "main"`) as three workspace packages: `canvas`, `render`, `tui` — not crates.io. Braille rendering needs subpixel precision; use line-based drawing for the TUI per the geometry crate's conventions.
