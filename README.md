# Tile

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
- `tilectl` CLI over a named pipe: dispatch, query (JSON), subscribe to events
- Tray icon: pause, reload, open config/logs, quit
- Hidden windows and removed title bars are always restored: on exit, Ctrl-C and panic by the
  daemon itself, and on a hard kill by a small watchdog process (`tile --watchdog <pid>`)

## Usage

```powershell
cargo build --release
.\target\release\tile.exe            # reads %APPDATA%\Tile\config.toml if it exists
.\target\release\tile.exe -c my.toml # alternative config
```

Logs go to `%LOCALAPPDATA%\Tile\logs`. Run elevated if you want admin windows tiled too.

```powershell
tilectl dispatch workspace 3
tilectl dispatch layout scrolling
tilectl query windows --json
tilectl subscribe        # JSON lines: workspace / active_window / layout changes
tilectl reload
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

Bound keys take priority over Windows' own `Win+` shortcuts. `Win+L` is special (see
`override_win_l` below).

## Configuration

The config file is optional: without it, the built-in defaults above apply. Any value you set
overrides its default; a `[binds]` or `[[rules]]` table replaces the defaults entirely. Saved
changes are applied live.

```toml
[general]
gaps_in = 5
gaps_out = 10
layout = "dwindle"            # dwindle | master | monocle | scrolling
borders = true                # Windows 11 DWM border colors
border_active = "#89b4fa"
border_inactive = "#45475a"
focus_new_windows = true
decorations = false           # title bars on tiled windows (restored on exit/crash)
override_win_l = false        # take over Win+L; lock with the `lock` dispatcher instead
disable_click_to_do = true    # switch off Click to Do (glow on holding Win) while Tile runs

[animations]
enabled = true
duration_ms = 150
bezier = [0.05, 0.9, 0.1, 1.0]

[dwindle]
split_ratio = 0.5

[master]
mfact = 0.55
count = 1
new_is_master = false

[scrolling]
proportion = 0.5
proportion_presets = [0.333, 0.5, 0.667, 1.0]
center_focused = "on_overflow" # always | on_overflow | never

[binds]                       # "<super|shift|ctrl|alt>+<key>" = "<dispatcher> [args]"
"super+enter" = "exec wt.exe"

[[rules]]                     # case-insensitive regexes; all given matchers must match
exe = "^Spotify\\.exe$"       # also: class, title
workspace = 9                 # also: float, monitor, decorations, ignore
```

Dispatchers: `exec`, `killactive`, `togglefloating`, `fullscreen`, `movefocus`, `movewindow`,
`resizeactive`, `swapwithmaster`, `cyclenext`, `cycleprev`, `togglesplit`, `workspace`,
`movetoworkspace`, `movetoworkspacesilent`, `focusmonitor`, `layout`, `cyclelayout`,
`scroller_proportion`, `cycleproportion`, `consumewindow`, `expelwindow`, `centercolumn`,
`desktop`, `movetodesktop`, `pin`, `lock`, `reload`, `togglepause`, `exit`.

**Win+L:** Windows handles it below any keyboard hook. With `override_win_l = true`, Tile
sets the per-user `DisableLockWorkstation` policy while it runs (restored on exit, crash or kill).
If that policy key is admin-only on your machine, Tile asks once via UAC to let your account
manage that single key.

**Minimum sizes:** apps that refuse a tile smaller than their minimum size are resized with
`SWP_NOSENDCHANGING`, so they fit their tile anyway.

## Layout

| Crate | Purpose |
| --- | --- |
| `tile-core` | Platform-independent state, layouts, commands, easing (unit tested) |
| `tile-config` | TOML schema, keybind parsing, window rules, file watching |
| `tile-platform` | Win32 integration: window events, placement, hotkeys, tray |
| `tile-ipc` | Named-pipe JSON protocol |
| `tile` | The daemon |
| `tilectl` | The CLI |
