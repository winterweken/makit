# Repository Guidelines

## Project Structure & Module Organization
- `crates/makit-cli/`: Binary crate — clap CLI entrypoint with `list`, `exec`, `analyze`, `tui`, `status`, `init` subcommands.
- `crates/makit-core/`: Registry singleton, config loading (figment/YAML), and core model types (`Tool`, `Category`, `Task`, `Source`, `Action`).
- `crates/makit-geometry/`: Geometry primitives (`Point`, `Line`, `Rectangle`, `Room`, `Floor`) and braille drawing utilities (scanline fill, wall rendering).
- `crates/makit-tools/`: Tool implementations registered via `register_tasks()` — revit, rhino, blender, ifc, analysis, architect, murb.
- `crates/makit-tui/`: rsille-native TUI with tree explorer, braille canvas visualization, and Elm-like architecture (State/Msg/update/view).
- `examples/`: Canvas demo and sample IFC files; use these for reproducible tests.
- `pyrevit-extension/`: Packaged pyRevit extension (Python scripts and startup hooks — unchanged from Go era).
- `scripts/blender/`: Python Blender addon (unchanged).

## Build, Test, and Development Commands
- `cargo build`: Build the entire workspace.
- `cargo build --release`: Production build.
- `cargo run -p makit -- --help`: Smoke-test the binary and check available commands.
- `cargo run -p makit -- tui`: Launch the interactive TUI.
- `cargo run -p makit -- list`: List all registered tools/sources/actions.
- `cargo run -p makit --example canvas_demo`: Run the braille canvas demo.
- `cargo test`: Run the full test suite across all crates (15+ tests).
- `cargo fmt`: Enforce standard Rust formatting; required for all changes.
- `cargo clippy`: Run lints; address all warnings before opening a PR.

## Coding Style & Naming Conventions
- Use standard Rust style: snake_case for functions/variables, PascalCase for types, and doc comments (`///`) on public items.
- Keep CLI command names short (`list`, `exec`, etc.); follow existing clap derive patterns in `makit-cli/src/commands/`.
- Favor pure functions in `makit-geometry` and thin orchestration in `makit-cli/src/commands/` to keep logic testable.
- Use `anyhow::Result` for CLI/tool errors; use `thiserror` for library-level typed errors.
- rsille TUI follows Elm architecture: `State` struct, `Msg` enum, `update(state, msg)`, `view(state) -> Widget`.
- When touching Python in `pyrevit-extension/`, mirror existing naming (`*_extractors.py`, `*_engine.py`) and keep functions snake_case.

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
