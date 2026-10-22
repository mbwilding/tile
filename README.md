# tiling

A Hyprland / mangowm style dynamic tiling window manager for Windows, written in Rust with
**zero `unsafe`** in this repository (`unsafe_code = "forbid"` in every crate). Win32 access goes
through safe wrappers: [`winsafe`](https://crates.io/crates/winsafe),
[`wineventhook`](https://crates.io/crates/wineventhook),
[`win-hotkeys`](https://crates.io/crates/win-hotkeys),
[`uiautomation`](https://crates.io/crates/uiautomation) and
[`tray-icon`](https://crates.io/crates/tray-icon).

No external hotkey daemon is needed — keybinds are handled in-process via a low-level keyboard
hook, including `Win` combos (the Start menu is suppressed).

## Features

- Layouts per workspace: **dwindle**, **master/stack**, **monocle**, **scrolling** (niri/mangowm style columns)
- Hyprland-style workspaces 1–9 across multiple monitors, floating, fullscreen
- Native virtual desktop aware (via [`winvd`](https://crates.io/crates/winvd), Windows 11 24H2+):
  every desktop has its own workspaces and layouts; Task View, `Win+Ctrl+Arrows` and
  "show on all desktops" keep working
- Mouse: drag a tile onto another to swap, resize a tile's edge to resize the split
- Focus borders (Windows 11) and bezier-eased move/resize animations
- Optional title bar removal for tiled windows (`decorations = false`, per-app override via rules)
- TOML config with live reload and window rules (float / workspace / monitor / decorations / ignore)
- `tilingctl` CLI over a named pipe: dispatch, query (JSON), subscribe to events
- Tray icon: pause, reload, open config/logs, quit
- Hidden windows and removed title bars are always restored: on exit, Ctrl-C and panic by the
  daemon itself, and on a hard kill by a small watchdog process (`tiling --watchdog <pid>`)

## Usage

```powershell
cargo build --release
.\target\release\tiling.exe            # uses %APPDATA%\tiling\config.toml (created on first run)
.\target\release\tiling.exe -c my.toml # alternative config
```

Logs go to `%LOCALAPPDATA%\tiling\logs`. Run elevated if you want admin windows tiled too.

```powershell
tilingctl dispatch workspace 3
tilingctl dispatch layout scrolling
tilingctl query windows --json
tilingctl subscribe        # JSON lines: workspace / active_window / layout changes
tilingctl reload
```

## Default keybinds

| Keys | Action |
| --- | --- |
| `Win+Enter` | open Windows Terminal |
| `Win+Q` | close window |
| `Win+V` / `Win+F` | toggle floating / fullscreen |
| `Win+Space` | cycle layout |
| `Win+Arrows` | move focus |
| `Win+Shift+Arrows` | move window (to the next monitor at the edge) |
| `Win+Alt+Arrows` | resize |
| `Win+Ctrl+Left/Right`, `Win+Tab` | native: switch virtual desktop, Task View |
| `Win+Ctrl+Shift+Left/Right` | move window to previous / next virtual desktop |
| `Win+Ctrl+P` | pin window to all desktops (it floats, untiled) |
| `Win+1..9` / `Win+Shift+1..9` | switch to / move window to workspace |
| `Win+,` / `Win+.` | focus monitor left / right |
| `Win+M` / `Win+T` | swap with master / toggle dwindle split |
| `Win+R` / `Win+C` | scrolling: cycle column width / center column |
| `Win+[` / `Win+]` / `Win+\` | scrolling: consume into left / right column, expel |
| `Win+Shift+R` / `Win+Shift+P` / `Win+Shift+E` | reload config / pause / exit |

`Win+L` is reserved by Windows and can't be bound. See [`config/default.toml`](config/default.toml)
for every option and the full dispatcher list.

## Layout

| Crate | Purpose |
| --- | --- |
| `tiling-core` | Platform-independent state, layouts, commands, easing (unit tested) |
| `tiling-config` | TOML schema, keybind parsing, window rules, file watching |
| `tiling-platform` | Win32 integration: window events, placement, hotkeys, tray |
| `tiling-ipc` | Named-pipe JSON protocol |
| `tiling` | The daemon |
| `tilingctl` | The CLI |
