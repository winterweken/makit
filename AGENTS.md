# Repository Guidelines

## Information Recording Principle

This document uses **progressive disclosure**. Level 1 (this file) is loaded every conversation. Level 2 (`docs/`) is loaded on demand.

### Level 1 (this file) records
| Type | Example |
|------|---------|
| Core commands | `cargo test`, `cargo run -p makit -- tui` |
| Module map | crate → purpose → key files |
| Code patterns | Elm architecture, error handling |
| Commit/PR rules | Imperative messages, verification steps |

### Level 2 (`docs/`) records
| Type | Example |
|------|---------|
| Architecture deep-dives | Hybrid 3-layer bridge design |
| Analysis algorithms | Wall orientation, WWR calculation |
| Format specifications | IFC extraction, JSON schema |

### When user asks to record information
1. High-frequency or violation-critical → this file (Level 1)
2. Detailed SOP, algorithm, or reference → `docs/` (Level 2) + trigger in this file

---

## Reference Index

| Trigger | Document | Contains |
|---------|----------|----------|
| Hybrid bridge architecture, 3-layer design | `docs/HYBRID_ARCHITECTURE.md` | Layer diagram, JSON format spec, extraction/analysis/CLI flow |
| IFC file analysis, `analyze_ifc.py` | `docs/IFC_SUPPORT.md` | Standalone Python usage, storey filtering, unit config |
| Wall orientation, WWR, compass bucketing | `docs/WALL_ORIENTATION_ANALYSIS.md` | atan2 logic, cardinal direction math, Revit/IFC extractors |

---

## Project Structure & Module Organization
- `crates/makit-cli/`: Binary crate — clap CLI entrypoint with `list`, `exec`, `analyze`, `tui`, `status`, `init` subcommands.
- `crates/makit-core/`: Registry singleton, config loading (figment/YAML), and core model types (`Tool`, `Category`, `Task`, `Source`, `Action`).
- `crates/makit-geometry/`: Geometry primitives (`Point`, `Line`, `Rectangle`, `Room`, `Floor`), drawing utilities generic over the `Plotter` trait (`plot.rs`; scanline fill, wall rendering, isometric projection), a dependency-free `BrailleGrid` (`braille.rs`, y-up, used by examples), `Recording` (replayed by the TUI), and SDF engine (`sdf.rs`). No external dependencies.
- `crates/makit-tools/`: Tool implementations registered via `register_tasks()`:
  - `murb.rs` — MURB energy modelling bridge (Python subprocess, JSON IPC)
  - `revit/` — Revit HTTP bridge (`client.rs` async reqwest, `models.rs` wall/floor/room types, `mod.rs` handlers)
  - `blender.rs` — Blender geometry sync server (axum, `Arc<RwLock>` shared state)
  - `ifc.rs`, `rhino.rs`, `analysis.rs`, `architect.rs` — other tool registrations
- `crates/makit-tui/`: ratatui 0.29 + crossterm 0.28 TUI, Elm architecture: `app.rs` (AppState), `msg.rs` (Msg/Cmd), `update.rs` (pure), `view.rs`, `layout.rs` (shared by view + mouse hit-tests), `keymap.rs`, `runtime.rs` (event loop, worker threads), `terminal.rs` (stderr backend, panic hook), `capture.rs` (stdout → Output pane), `node.rs` (typed `NodeId`, `PreviewKind` routing), `tree.rs`, `task.rs`, `preview.rs`.
- `examples/`: Canvas demo and sample IFC files; use these for reproducible tests.
- `pyrevit-extension/`: Packaged pyRevit extension (Python scripts and startup hooks).
- `scripts/`: `blender/` Python Blender addon; `murb_runner.py` Python-side energy bridge.

## Build, Test, and Development Commands
- `cargo build`: Build the entire workspace.
- `cargo build --release`: Production build.
- `cargo run -p makit -- --help`: Smoke-test the binary and check available commands.
- `cargo run -p makit -- tui`: Launch the interactive TUI.
- `cargo run -p makit -- list`: List all registered tools/sources/actions.
- `cargo run -p makit --example canvas_demo`: Run the braille canvas demo.
- `cargo test`: Run the full test suite across all crates (157 tests: 8 core, 40 geometry, 27 tools, 82 tui).
- `cargo fmt`: Enforce standard Rust formatting; required for all changes.
- `cargo clippy`: Run lints; address all warnings before opening a PR.

## Coding Style & Naming Conventions
- Use standard Rust style: snake_case for functions/variables, PascalCase for types, and doc comments (`///`) on public items.
- Keep CLI command names short (`list`, `exec`, etc.); follow existing clap derive patterns in `makit-cli/src/commands/`.
- Favor pure functions in `makit-geometry` and thin orchestration in `makit-cli/src/commands/` to keep logic testable.
- Use `anyhow::Result` for CLI/tool errors; use `thiserror` for library-level typed errors.
- ratatui TUI follows Elm architecture: `AppState` struct, `Msg` enum, `update(&mut state, msg) -> Vec<Cmd>` (pure — side effects go through `Cmd` and run in `runtime.rs`), `view(frame, &state)`. Never `println!` from TUI code: stdout is captured into the Output pane while the TUI runs.
- When touching Python in `pyrevit-extension/` or `scripts/`, mirror existing naming (`*_extractors.py`, `*_engine.py`) and keep functions snake_case.

## Testing Guidelines
- Place tests alongside code as `#[cfg(test)] mod tests` within each module.
- Use assets under `examples/` for deterministic inputs; avoid hardcoding user-specific paths.
- Include regression tests when changing registry/task wiring or CLI flags.
- Document any platform-specific assumptions (Windows/macOS/Linux) in test names or comments.

## Commit & Pull Request Guidelines
- Commit messages: short, imperative summaries (e.g., `Add TUI navigation guard`, `Fix code quality issues and bugs`) similar to existing history.
- PRs should include: what changed, why, how to verify (commands run with output snippets), and any new flags/config keys.
- Update `README.md` or `docs/` when adding user-facing commands or config fields; note required pyRevit/Revit versions if applicable.
- Link related issues and attach screenshots or terminal recordings for TUI/UX changes when possible.

---

## Before Modifying Code

| You want to change | Read first | Key trap |
|--------------------|-----------|----------|
| Revit bridge / extraction | `docs/WALL_ORIENTATION_ANALYSIS.md` | atan2 axis convention; pyRevit HTTP on port 48884 |
| IFC parsing / analysis | `docs/IFC_SUPPORT.md` | IfcOpenShell import; storey filtering edge cases |
| Bridge architecture / JSON schema | `docs/HYBRID_ARCHITECTURE.md` | 3-layer separation; generic format is platform-agnostic |
| TUI canvas / preview rendering | `crates/makit-tui/src/preview.rs`, `node.rs` | Routing is `PreviewKind::for_node` (matches the task name, not option names); previews are drawn into a `Recording` and scaled by `fit_bounds` |
| TUI layout / mouse | `crates/makit-tui/src/layout.rs` | `view` and `keymap` must both use `layout::compute` so clicks hit what is drawn |
| Geometry / SDF / isometric | `crates/makit-geometry/src/` | SDF needs subpixel for braille; use line-based for TUI |
| Registry / task wiring | `crates/makit-core/src/registry.rs` | Global singleton; `register_tasks()` called at startup |
| MURB energy bridge | `scripts/murb_runner.py` + `crates/makit-tools/src/murb.rs` | Subprocess JSON IPC; needs `murb_energy_tool` Python package |

---

## Reference Trigger Index

| Trigger | Document |
|---------|----------|
| Hybrid bridge architecture | `docs/HYBRID_ARCHITECTURE.md` |
| IFC file analysis | `docs/IFC_SUPPORT.md` |
| Wall orientation / WWR | `docs/WALL_ORIENTATION_ANALYSIS.md` |
