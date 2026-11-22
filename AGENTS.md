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
- `crates/makit-tui/`: ratatui 0.29 + crossterm 0.28 TUI, Elm architecture: `app.rs` (AppState, option values, `Editor` form), `msg.rs` (Msg/Cmd), `update.rs` (pure), `view.rs`, `layout.rs` (shared by view + mouse hit-tests), `keymap.rs`, `runtime.rs` (event loop, worker threads, cancel tokens), `terminal.rs` (stderr backend, panic hook, terminal check), `capture.rs` (stdout → Output pane, task results in output order), `signals.rs` (restore the terminal on SIGTERM), `node.rs` (typed `NodeId`, `PreviewKind` routing, picker/results bindings), `options.rs` (option `Catalog` copied from the registry), `help.rs` (help text), `logo.rs` (the makit symbol: header icon, braille preview, `mini`), `tree.rs`, `task.rs`, `preview.rs`.
- `docs/brand/`: The makit symbol as SVG (marks for dark and light backgrounds, app icons) and notes on using it.
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
- `cargo test`: Run the full test suite across all crates (279 tests: 10 core, 40 geometry, 47 tools, 182 tui).
- `cargo fmt`: Enforce standard Rust formatting; required for all changes.
- `cargo clippy`: Run lints; address all warnings before opening a PR.
- `.venv/bin/python -m unittest discover -s pyrevit-extension/tests`: Python IFC extractor and analysis tests (13). They need `ifcopenshell`; set up `.venv` once with `uv venv .venv --python 3.13 && uv pip install --python .venv/bin/python ifcopenshell`.
- `python3 -m unittest discover -s scripts/tests`: MURB runner tests (2); they stand in for `murb_energy_tool`, so they need no extra packages.

## Coding Style & Naming Conventions
- Use standard Rust style: snake_case for functions/variables, PascalCase for types, and doc comments (`///`) on public items.
- Keep CLI command names short (`list`, `exec`, etc.); follow existing clap derive patterns in `makit-cli/src/commands/`.
- Favor pure functions in `makit-geometry` and thin orchestration in `makit-cli/src/commands/` to keep logic testable.
- Use `anyhow::Result` for CLI/tool errors; use `thiserror` for library-level typed errors.
- ratatui TUI follows Elm architecture: `AppState` struct, `Msg` enum, `update(&mut state, msg) -> Vec<Cmd>` (pure — side effects go through `Cmd` and run in `runtime.rs`), `view(frame, &state)`. Never `println!` from TUI code: stdout is captured into the Output pane while the TUI runs (the one deliberate exception is `capture::announce_done`, the end-of-output marker).
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
| Revit bridge / extraction | `docs/WALL_ORIENTATION_ANALYSIS.md` | atan2 axis convention; pyRevit HTTP on port 48884; a wall's `orientation` is its outward normal in project coordinates and `projectNorth` turns it to true north (both Python and Rust apply it); an IFC wall's local X runs along the wall; walls with `isExternal: false` (and their windows) are left out of the analysis |
| IFC parsing / analysis | `docs/IFC_SUPPORT.md` | IfcOpenShell import; storey filtering edge cases; scale quantities by the file's units (`get_unit_scales`), and Revit writes a bogus mm² `NetSideArea` before the real one |
| Bridge architecture / JSON schema | `docs/HYBRID_ARCHITECTURE.md` | 3-layer separation; generic format is platform-agnostic |
| TUI canvas / preview rendering | `crates/makit-tui/src/preview.rs`, `node.rs` | Routing is `PreviewKind::for_node` (matches the task name, not option names); previews are drawn into a `Recording` and scaled by `fit_bounds`; the logo preview is coloured from `logo::segments` in `view.rs`, so change its shape in `logo.rs` |
| TUI layout / mouse | `crates/makit-tui/src/layout.rs` | `view` and `keymap` must both use `layout::compute` so clicks hit what is drawn; on short screens `detail_rows` drops the preview whole and the picker may be absent (then it cannot take focus); list scroll offsets are set in one place, `sync_scroll` at the end of `update`, because the mouse hit-tests read them |
| TUI option values / editor | `crates/makit-tui/src/options.rs`, `app.rs` (`option_value`), `update.rs` | A value resolves user → IFC picker → registered default; `update` reads the `Catalog`, never the registry; the editor and help are modal (checked first in `update` and `keymap`) |
| TUI task results / stop | `crates/makit-tui/src/capture.rs`, `runtime.rs` | Results go through the stdout pipe (`Mailbox` + marker) so they cannot overtake output; long-running handlers must poll `ctx.cancel` to be stoppable |
| TUI signals / shutdown | `crates/makit-tui/src/signals.rs`, `lib.rs` | The event loop may never see the signal flag (crossterm's poll spins once the tty is gone); the watchdog thread must stay able to end the process by itself |
| Geometry / SDF / isometric | `crates/makit-geometry/src/` | SDF needs subpixel for braille; use line-based for TUI |
| Registry / task wiring | `crates/makit-core/src/registry.rs` | Global singleton; `register_tasks()` called at startup |
| MURB energy bridge | `scripts/murb_runner.py` + `crates/makit-tools/src/murb.rs` | Subprocess JSON IPC; needs `murb_energy_tool` on the Python path (stdlib-only, not pip-installable: set `PYTHONPATH` to the murb repo); its results are `Vec` lists, not numpy arrays; `murb-simulate --window-directions` (`azimuth:share` pairs, e.g. `S:60,W:40`) splits the glazing by facing; without it `--model` (IFC or extraction; the IFC picker fills it) takes the split from windows in exterior walls (`building_model::window_area_by_facing`; curtain-wall glass is not extracted yet); with neither, 25% each way |

---

## Reference Trigger Index

| Trigger | Document |
|---------|----------|
| Hybrid bridge architecture | `docs/HYBRID_ARCHITECTURE.md` |
| IFC file analysis | `docs/IFC_SUPPORT.md` |
| Wall orientation / WWR | `docs/WALL_ORIENTATION_ANALYSIS.md` |
