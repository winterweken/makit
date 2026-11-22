# makit symbol

A 2×2×2 kit of blocks with the front-top block lifted out of its socket: the one being made. The kit is slate; the lifted block carries the TUI's three theme colours (top `accent` #FFB450, left `primary` #5AC8FA, right `secondary` #B482FF).

| File | Use |
| --- | --- |
| `makit-mark-dark.svg` | README and docs on dark backgrounds |
| `makit-mark-light.svg` | README and docs on light backgrounds |
| `makit-icon.svg` | App icon, social avatar (48 px and up) |
| `makit-icon-small.svg` | Favicon and anything 32 px or smaller: just the lifted block |
| `makit-cube.svg` | The block on its own, transparent |

In the TUI (`crates/makit-tui/src/logo.rs`):

- **Header:** a two-cell pixel cube, `▄▄` with an amber background and cyan / purple foregrounds.
- **Preview:** the full symbol as a hidden-line braille wireframe; the block hovers with the animation angle and never lands.
- **`logo::mini()`:** a 4×2-cell hexagonal cube (`◢██◣` / `◥██◤`) for places with a little more room.

Keep the block lifted. At 32 px and below, drop the kit and use the block alone.
