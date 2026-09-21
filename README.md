# hntui

Hacker News in your terminal! (formerly published as `@ahmd-sh/hackernuis`)

```
bun add -g hntui
hntui
```

## What it does

- Browse all six HN feeds: Top, New, Best, Ask, Show, and Jobs.
- Navigate through a post's comments.
- Save posts for later.
- Vim + mouse support.
- Themes.
- ... and a really cool Knight Rider scanner animation for loaders (⁠◕⁠ᴗ⁠◕⁠ )

## Requirements

You need [Bun](https://bun.sh) 1.2 or newer. Install it with:

```bash
curl -fsSL https://bun.sh/install | bash
```

Any modern terminal with truecolor, mouse support, and UTF-8 will work. I've tested it in Ghostty on MacOS. Linux/Windows is supported by OpenTUI but I haven't tried it (yet).

## Install

```bash
bun add -g hntui
```

Or run it once without installing:

```bash
bunx hntui
```

## Run

```bash
hntui
```

Press `q` (or `Ctrl-C`) to quit.

## Keybindings

### Story list

| Key | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Move cursor |
| `gg`, `Shift-G` | Jump to first or last |
| `Ctrl-D`, `Ctrl-U`, `PgDown`, `PgUp` | Scroll a half page |
| `c`, `Enter` | Open the story and read its comments |
| `h` / `←`, `l` / `→` | Previous or next tab |
| `Tab`, `Shift-Tab` | Cycle through tabs |
| `1` through `6` | Jump to a specific category |
| `Shift-S` | Jump to the Saved list |
| `s` | Save or unsave the highlighted post |
| `o` | Open the story's URL in your browser |
| `r` | Refresh the current feed |
| `t` | Toggle theme |
| `q`, `Ctrl-C` | Quit |

### Story detail (comments)

| Key | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Move the comment cursor |
| `gg`, `Shift-G` | Jump to first or last comment |
| `Ctrl-D`, `Ctrl-U`, `PgDown`, `PgUp` | Scroll a half page |
| `Space` | Collapse or expand the current subtree |
| `Enter` | Open the links popup for the current comment |
| `s` | Save or unsave this story |
| `o` | Open the story's URL |
| `Esc`, `Backspace`, `h` / `←` | Back to the list |
| `t` | Toggle theme |
| `q` | Quit |

### Links popup

| Key | Action |
|---|---|
| `j`, `k`, `↑`, `↓` | Move |
| `gg`, `Shift-G` | First or last link |
| `o`, `Enter` | Open the highlighted link |
| `Esc`, `Backspace` | Close the popup |

### Context menu (right-click)

| Key | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Move |
| `Enter` | Activate |
| `Esc`, `Backspace` | Close |

### Mouse

Most things you can do with the keyboard, you can do with a mouse too.

- Click a tab to switch feeds.
- Click the `Y` tile to refresh the current feed (or to exit a story back to its list).
- Click any story row to select it. Click it again to open the comments.
- Right-click a story to open a context menu with Save, Open URL, and Open Comments.
- Click a comment's header line to collapse or expand its subtree.
- Double-click a comment's body to open its links popup.
- Click the story URL in the detail header to open it in your browser.
- Click outside a popup or context menu to dismiss it.
- Use your scroll wheel to scroll lists and comment trees.

### Selecting and copying text

Because the app captures mouse events, your terminal's normal click-and-drag selection is intercepted. To select text the regular way:

- On macOS (Terminal.app, iTerm2, WezTerm, Ghostty), hold `Option` while you drag, then `Cmd-C`.
- On Linux (Kitty, Alacritty, WezTerm, GNOME Terminal), hold `Shift` while you drag, then `Ctrl-Shift-C`.

## Themes

Press `t` to toggle. The dark theme is mostly black with orange accents. The light theme is faithful to news.ycombinator.com: white background, orange topbar, the familiar beige row highlight, and HN's classic grey byline text.

## Saved posts

Press `s` on any story to save it. Saved posts get a small star next to the title and show up in the Saved tab on the right side of the tab strip. The list persists across sessions in `~/.config/hntui/saved.json` as a small JSON file. (Config from the app's `hackernuis` days is migrated automatically on first run.) Press `s` again to remove a post from the list.

`Shift-S` jumps straight to the Saved list from anywhere.

## Development

```bash
git clone https://github.com/ahmd-sh/hntui.git
cd hntui
bun install
bun dev    # hot reload
```

Data comes from the public [Hacker News Firebase API](https://github.com/HackerNews/API).

## Releases

This package publishes to npm via GitHub Actions using OIDC trusted publishing. Every release has a [SLSA provenance attestation](https://slsa.dev/) linking the tarball back to the exact commit and workflow run that produced it. To release a new version:

```bash
npm version patch   # or minor / major
git push --follow-tags
```

The `v*` tag triggers `.github/workflows/publish.yml` which runs `npm publish --provenance --access public`.

## Acknowledgments

- [OpenTUI](https://github.com/anomalyco/opentui) by Anomaly. The native TUI core that makes all of this possible.
- [opentui-spinner](https://github.com/msmps/opentui-spinner) by Matt Simpson. The Knight Rider loading scanner is adapted from `examples/knight-rider/utils.ts` (MIT).
- [Hacker News](https://news.ycombinator.com) for the content and the open API.

## License

[MIT](./LICENSE), Copyright (c) 2026 Ahmed Shaikh.
