# Changelog

## 0.6.0

hntui has been rewritten in Rust. It looks and works the same and no longer needs Bun.

| | 0.5.1 | 0.6.0 |
|---|---|---|
| Binary size | 65 MB | 3.6 MB |
| Time to first screen | ~72 ms | ~17 ms |
| Memory, story list | ~91 MB | ~17 MB |
| Memory, a 430-comment thread | ~200 MB | ~54 MB |
| CPU while scrolling that thread | ~78% | ~30% |
| CPU while idle | ~0.3% | 0% |

(measured on an Apple Silicon Mac)

### New

- A config file, `~/.config/hntui/config.toml`. `hntui config` to show, `hntui config edit` to open.
- `mouse = false` to leave the mouse to your terminal.
- `Y` to copy what's in focus.

### Changed

- The npm package carries the native binary now.
- Dragging inside the app no longer highlights text. You can use `Y`, hold a modifier while you drag (usually Shift, depending on your terminal), or set `mouse = false` in config.

### Fixed

- Old comments with stray Windows characters show them properly (`don’t`) instead of garbling the screen.
- Folding a comment by clicking its header no longer makes the view jump.
- On short terminals, the help and links popups fit the screen, and the links popup scrolls to the selected link.

Older releases: https://github.com/ahmd-sh/hntui/releases
