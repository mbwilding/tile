# Tile

A dynamic tiling window manager for Windows, written in Rust with **zero `unsafe`** in this
repository (`unsafe_code = "forbid"` in every crate). Win32 access goes through safe wrappers:
[`winsafe`](https://crates.io/crates/winsafe),
[`wineventhook`](https://crates.io/crates/wineventhook),
[`win-hotkeys`](https://crates.io/crates/win-hotkeys),
[`uiautomation`](https://crates.io/crates/uiautomation),
[`tray-icon`](https://crates.io/crates/tray-icon) and
[`winvd`](https://crates.io/crates/winvd).

Keybinds are handled in-process by a low-level keyboard hook, including `Win` combos (the Start
menu is suppressed), so no separate hotkey program is needed.

## Features

- Four layouts, chosen per workspace: **dwindle**, **master/stack**, **monocle** and
  **scrolling** (an endless strip of columns)
- Numbered workspaces across multiple monitors; floating and fullscreen windows
- Native virtual desktops (Windows 11 24H2+): every desktop has its own workspaces and layouts;
  Task View, `Win+Ctrl+Left/Right` and "show this window on all desktops" keep working
- Mouse: drag a tile onto another to swap them, drag a tile's edge to resize the split
- Coloured focus borders (Windows 11) and eased move/resize animations, paced by each app's own
  redraws
- Optional title bar removal for tiled windows, with per-app overrides
- TOML config, applied live on save, with window rules
- `tilectl`: dispatch commands, query state as JSON, subscribe to events, install/uninstall
- Tray icon: pause, reload, open config or logs, quit
- Everything Tile changes is undone when it stops, including on a crash or a hard kill (a small
  watchdog process handles that)

## Install and run

```powershell
cargo build --release --workspace    # builds tile.exe and tilectl.exe
.\target\release\tilectl install     # install and start Tile (one UAC prompt)
```

`tilectl install` sets Tile up to run as administrator, so it can manage admin windows too,
without a UAC prompt at sign-in:

- It copies both programs to `C:\Program Files\Tile`, where only admins can change them.
- A scheduled task starts Tile elevated when you sign in. It runs on battery, is never stopped
  for running too long and runs at normal priority.
- A launcher (`tile.exe --launcher`) starts with your normal rights from your `Run` key. Programs
  Tile starts, from `exec` binds or the tray, go through it, so they don't run as administrator.
- `C:\Program Files\Tile` goes on your user `PATH`, for `tilectl` in terminals started after your
  next sign-in.

It replaces a running Tile and asks for admin rights once. To update after a rebuild, run
`tilectl install` again from the checkout: the installed `tilectl` installs the build in
`.\target\release`. Running programs never block an update, since the old files are moved aside
and deleted on the next install. `tilectl uninstall` stops Tile and undoes all of that, keeping
your config and logs.

To try a build without installing:

```powershell
cargo run --release                  # runs tile.exe
.\target\release\tile.exe -c my.toml # with another config file
```

Only one Tile runs at a time. Release builds have no console, so stop one with `Win+Shift+E`, the
tray icon or `tilectl dispatch exit`; Ctrl-C in the terminal only stops `cargo`. `tile.exe
--log tile=debug` logs every keybind it handles.

| Path | Contents |
| --- | --- |
| `%APPDATA%\Tile\config.toml` | Config, created on first run |
| `%LOCALAPPDATA%\Tile\logs` | Daily log files |
| `%LOCALAPPDATA%\Tile\restore.json` | What to undo if Tile dies, while it runs |
| `C:\Program Files\Tile` | The installed programs |

## Default keybinds

| Keys | Action |
| --- | --- |
| `Win+Enter` | open Windows Terminal |
| `Win+Q` | close window |
| `Win+V` / `Win+F` | toggle floating / fullscreen |
| `Win+Space` | next layout |
| `Win+Arrows` / `Win+H/J/K/L` | move focus |
| `Win+Shift+Arrows` / `Win+Shift+H/J/K/L` | move window (to the next monitor at the edge) |
| `Win+Alt+Arrows` / `Win+Alt+H/J/K/L` | resize |
| `Win+1..9` / `Win+Shift+1..9` | switch to / move window to workspace |
| `Win+,` / `Win+.` | focus monitor left / right |
| `Win+M` / `Win+T` | swap with master / flip the dwindle split |
| `Win+R` / `Win+C` | scrolling: next column width / center column |
| `Win+[` / `Win+]` / `Win+\` | scrolling: merge into left / right column, split out |
| `Win+Ctrl+Shift+Left/Right` | move window to previous / next virtual desktop |
| `Win+Ctrl+P` | show window on all desktops (it floats) |
| `Win+Shift+R` / `Win+Shift+P` / `Win+Shift+E` | reload config / pause / exit |

`Win+Ctrl+Left/Right`, `Win+Ctrl+D` and `Win+Tab` are left to Windows' virtual desktops. Bound
keys take priority over Windows' own `Win+` shortcuts; unbound ones still reach Windows.

## Configuration

On first run Tile writes `%APPDATA%\Tile\config.toml` with every default listed but commented
out. Uncomment a line, together with its `[section]` header, to change it; saved changes apply
immediately. A `[binds]` table or `[[rules]]` list replaces the defaults entirely, so the
generated file lists them all as a starting point.

```toml
[general]
gaps_in = 10                  # px between neighbouring windows
gaps_out = 10                 # px between windows and the monitor edge
layout = "dwindle"            # layout of new workspaces: dwindle | master | monocle | scrolling
borders = true                # colour the window border to show focus (Windows 11)
border_active = "#89b4fa"
border_inactive = "#45475a"
focus_new_windows = true
decorations = false           # keep title bars on tiled windows
override_win_l = true         # take over Win+L (see below)
disable_click_to_do = true    # switch off Click to Do (see below)

[animations]
enabled = true
duration_ms = 180
bezier = [0.0, 0.75, 0.15, 1.0] # cubic-bezier control points (x1, y1, x2, y2)

[dwindle]
split_ratio = 0.5             # share of a split given to the existing window

[master]
mfact = 0.55                  # width of the master area
count = 1                     # windows in the master area
new_is_master = false         # new windows become master

[scrolling]
proportion = 0.5              # width of new columns, as a share of the monitor
proportion_presets = [0.333, 0.5, 0.667, 1.0] # widths `cycleproportion` steps through
center_focused = "on_overflow" # always | on_overflow | never

[binds]
"super+enter" = "exec wt.exe"
"super+shift+q" = "killactive"

[[rules]]
exe = "^Spotify\\.exe$"
workspace = 9
```

### Keybinds

A bind is `"<modifiers>+<key>" = "<dispatcher> [arguments]"`. Modifiers are `super` (or `win`),
`shift`, `ctrl` and `alt`; a bind fires only when exactly those modifiers are held. Keys are
letters, digits, `f1`..`f24`, `left`/`right`/`up`/`down`, `space`, `tab`, `enter`, `esc`,
`backspace`, `delete`, `insert`, `home`, `end`, `pageup`, `pagedown`, `print`, `comma`, `period`,
`minus`, `equal`, `semicolon`, `slash`, `grave`, `bracketleft`, `bracketright`, `backslash`,
`apostrophe`, or any Windows virtual-key name such as `OEM_102`.

| Dispatcher | Action |
| --- | --- |
| `exec <command line>` | run a program |
| `killactive` | close the active window |
| `togglefloating` / `fullscreen` | toggle floating / fullscreen |
| `movefocus <l\|r\|u\|d>` | focus the nearest window that way (or the next monitor) |
| `movewindow <l\|r\|u\|d>` | move the window that way, to the next monitor at the edge |
| `resizeactive <dx> <dy>` | grow (or shrink, with negative values) by pixels |
| `cyclenext` / `cycleprev` | focus the next / previous window in layout order |
| `workspace <n>` | switch to workspace `n` (1-255) |
| `movetoworkspace <n>` | move the window to workspace `n` and follow it |
| `movetoworkspacesilent <n>` | move the window to workspace `n` and stay |
| `focusmonitor <l\|r\|u\|d>` | focus the next monitor that way |
| `layout <name>` / `cyclelayout` | set / step through the workspace's layout |
| `swapwithmaster` | master: swap the window with the master |
| `togglesplit` | dwindle: flip the direction of the window's split |
| `scroller_proportion <0.05-1.0>` | scrolling: set the column's width |
| `cycleproportion` | scrolling: next width from `proportion_presets` |
| `consumewindow <l\|r>` | scrolling: move the window into the column that way |
| `expelwindow` | scrolling: move the window into a column of its own |
| `centercolumn` | scrolling: center the focused column |
| `desktop <n\|next\|prev>` | switch virtual desktop (past the last one creates one) |
| `movetodesktop <n\|next\|prev>` | send the window to another virtual desktop |
| `pin` | show the window on all virtual desktops (it floats) |
| `lock` | lock the computer (works while Win+L is taken over) |
| `reload` / `togglepause` / `exit` | reload the config / pause tiling / quit |

Directions can also be written `left`, `right`, `up` and `down`.

### Window rules

Each `[[rules]]` entry matches windows by `exe`, `class` and/or `title`, all case-insensitive
regular expressions; every matcher given must match. Rules apply in order, and a later match
overrides an earlier one. Actions:

| Action | Effect |
| --- | --- |
| `float = true` | float instead of tiling (or `false` to tile a window that would float) |
| `workspace = n` | open on workspace `n` |
| `monitor = n` | open on monitor `n` (from 0, left to right) |
| `decorations = true` | force the title bar on or off, overriding `general.decorations` |
| `ignore = true` | leave the window alone entirely |

`tilectl query windows` shows the `exe`, `class` and `title` of every managed window. The default
rules float standard dialogs and ignore shell windows, PowerToys and ShareX. Dialogs and
fixed-size windows float even without a rule.

## tilectl

```powershell
tilectl dispatch workspace 3      # run any dispatcher
tilectl query windows             # monitors | workspaces | windows | activewindow; --json for JSON
tilectl subscribe                 # JSON lines on workspace, active window and layout changes
tilectl reload                    # reload the config
tilectl install / uninstall       # see "Install and run"
```

It talks to Tile over the named pipe `\\.\pipe\tile.sock` (one JSON request per line, JSON
responses back), which scripts can use directly. Only your account (and administrators) can
connect, including from a normal terminal when Tile runs as administrator.

## Windows integration

**Win+L.** Windows handles Win+L before any keyboard hook sees it. With `override_win_l = true`,
the default, Tile sets the per-user `DisableLockWorkstation` policy while it runs, so Win+L can
be bound (to `movefocus r` by default) and the `lock` dispatcher locks instead. The policy is
removed when Tile stops, even after a crash or kill. If that policy is admin-only on your
machine and Tile doesn't run as administrator, it asks once through UAC to let your account
manage that single registry key. With `override_win_l = false`, Win+L locks as usual and binds
to it are skipped.

**Click to Do.** On Copilot+ PCs, holding Win shows Click to Do's glow around the pointer. Tile
switches it off once and leaves it off, because Windows shows a notice every time it is switched.
To get it back, set `disable_click_to_do = false` and turn it on in Settings, Privacy & security,
Click to Do.

**Admin windows.** A program running without admin rights can't move admin windows or see keys
pressed in them. The installed Tile runs as administrator, so it manages them like any other
window. A Tile started by hand without admin rights (e.g. `cargo run`) leaves them alone, and its
keybinds don't work while one has focus.

**Minimum sizes.** Apps that refuse a tile smaller than their minimum size are placed with
`SWP_NOSENDCHANGING`, so they fit their tile anyway.

**Undoing changes.** Tile hides windows on inactive workspaces, removes title bars and sets the
Win+L policy. It records these in `restore.json` and undoes them on exit and on panic; a watchdog
process (`tile.exe --watchdog <pid>`) undoes them if Tile is killed, and the next start cleans up
anything left over.

## TODO

- `Win` + left-drag anywhere in a window moves it; `Win` + right-drag resizes it in the
  direction the mouse moves, pushing the neighbouring tiles along

## Project layout

| Crate | Purpose |
| --- | --- |
| `tile-core` | Platform-independent state, layouts, dispatchers and easing (unit tested) |
| `tile-config` | TOML schema, keybind parsing, window rules, config template and file watching |
| `tile-platform` | Win32: window events, placement and animation, keyboard hook, tray, virtual desktops |
| `tile-ipc` | Named-pipe JSON protocol |
| `tile` | The daemon (`tile.exe`) |
| `tilectl` | The command-line tool (`tilectl.exe`), including install/uninstall |
