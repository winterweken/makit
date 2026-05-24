# Progress

- Fixed TUI caching by storing `tree_items` inside `AppState`.
- Implemented `Msg::Execute` logic to run tool/action handlers dynamically.
- Handled lock `.unwrap()` dynamically in `tree_data.rs`.
- Fixed error swallowing in `config.rs`.
- Protected `get_bounds()` against zero-dimension geometries.
- Added tests for config, draw_wall, and sdf_ring.

Last visited: 2026-05-23T11:22:53-04:00
