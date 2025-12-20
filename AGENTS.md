# Repository Guidelines

## Project Structure & Module Organization
- `cmd/makit/`: CLI entrypoint.
- `internal/cmd/`: Cobra commands (`list`, `exec`, `run`, `init`, `status`, `analyze`, `tui`).
- `internal/tools/`: Tool/task definitions (`revit`, `rhino`, `analysis`) registered via `registerTools()`.
- `internal/registry/`, `internal/config/`, `internal/pyrevit/`, `internal/tui/`: Registry core, config loading, pyRevit integration, and Bubble Tea UI.
- `pkg/geometry`, `pkg/canvas`, `pkg/utils`: Shared helpers; keep reusable logic here instead of duplicating inside commands.
- `docs/` and `examples/`: Reference material and sample IFC assets; use these for reproducible tests.
- `pyrevit-extension/`: Packaged pyRevit extension (Python scripts and startup hooks).

## Build, Test, and Development Commands
- `go build -o makit ./cmd/makit`: Build the CLI locally (Go 1.25+).
- `go run ./cmd/makit --help`: Smoke-test the binary and check available commands.
- `go test ./...`: Run the Go test suite; add table-driven tests for new logic.
- `go vet ./...`: Static checks for common mistakes; run before opening a PR.
- `gofmt -w .` (or `go fmt ./...`): Enforce standard formatting; required for all Go changes.

## Coding Style & Naming Conventions
- Use standard Go style: tabs for indentation, small lowercase package names, PascalCase for exported identifiers, and clear doc comments on exported types/functions.
- Keep CLI command names short (`list`, `exec`, etc.); follow existing Cobra patterns in `internal/cmd`.
- Favor pure functions in `pkg/` and thin orchestration in `internal/cmd/*` to keep logic testable.
- When touching Python in `pyrevit-extension/`, mirror existing naming (`*_extractors.py`, `*_engine.py`) and keep functions snake_case.

## Testing Guidelines
- Place tests alongside code as `*_test.go`; prefer table-driven cases and clear fixture names.
- Use assets under `examples/` for deterministic inputs; avoid hardcoding user-specific paths.
- Include regression tests when changing registry/task wiring or CLI flags.
- Document any platform-specific assumptions (Windows/macOS/Linux) in test names or comments.

## Commit & Pull Request Guidelines
- Commit messages: short, imperative summaries (e.g., `Add TUI navigation guard`, `Fix code quality issues and bugs`) similar to existing history.
- PRs should include: what changed, why, how to verify (commands run with output snippets), and any new flags/config keys.
- Update `README.md` or `docs/` when adding user-facing commands or config fields; note required pyRevit/Revit versions if applicable.
- Link related issues and attach screenshots or terminal recordings for TUI/UX changes when possible.
