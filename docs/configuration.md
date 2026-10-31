# Configuration

Tile reads `%APPDATA%\Tile\config.yml`, or `config.yaml` if that is the one there (`tilectl
config` prints the path). Tile runs on its built-in defaults, so the file only needs the settings
you want to change. On first run it creates `config.yml` holding just `tray`, the tray's
switches, which Tile saves there whenever you change them. Add a section and key to change
anything else; the [default config](#the-default-config) below lists them all to copy from. Saved changes apply immediately,
and `tilectl check [file]` reports what is wrong with a config. A bad config is not applied; the
previous one is kept.

Whole configs to start from are in [`examples/`](../examples): `default.yml` is the defaults
written out in full, every bind included (with `default_binds: false`), and `mbwilding.yml` is
the author's own Dvorak setup with home-row mods.

These sections **replace** the built-in set entirely when you write them, so copy the defaults
from the end of this page as a starting point:

- `rules`
- `remap.layers`

`binds` add to the [default binds](keybinds.md#default-keys) instead (see [`binds`](#binds)).
Every other section merges: a key you leave out keeps its default. Unknown keys are an error.

YAML needs quotes around a key or value that starts with punctuation (`"#89b4fa"`, `"/"`, `"!"`)
or contains `: ` or ` #`, and around numbers meant as text (`"2"`).

## Reference

### `general`

| Key | Default | Meaning |
| --- | --- | --- |
| `tiling` | `true` | Manage windows. `false` leaves them alone and gives Win+L back; turning it on again manages the open windows. |
| `default_binds` | `true` | Start from the [default binds](keybinds.md#default-keys), which `binds` add to. `false` leaves only your own `binds`. |
| `remap` | `true` | Run the key remapper with the `remap` layers. |
| `gaps_in` | `10` | Pixels between neighbouring tiled windows. |
| `gaps_out` | `10` | Pixels between tiled windows and the monitor edge. |
| `independent_workspaces` | `true` | Every monitor has workspaces of its own, so `workspace 2` is workspace 2 on the monitor you are on and never moves focus to another monitor. `false` syncs the monitors: switching workspace switches them all together. See [layouts](layouts.md#workspaces-and-monitors). |
| `layout` | `master` | Layout of new workspaces: `dwindle`, `master`, `monocle`, `scrolling` or `even`. New workspaces on a portrait monitor start with `even` instead. See [layouts](layouts.md). |
| `borders` | `true` | Colour the window border to show focus (Windows 11). |
| `border_active` | `"#89b4fa"` | Border colour of the focused window. |
| `border_inactive` | `"#45475a"` | Border colour of every other tiled window. |
| `focus_new_windows` | `true` | Focus a window when it opens. |
| `decorations` | `false` | Keep title bars on tiled windows. Floating windows always keep theirs; a rule can override this per app. |
| `override_win_l` | `true` | Take over Win+L so it can be bound. See [Windows integration](windows-integration.md). |
| `disable_click_to_do` | `true` | Switch off Click to Do, whose glow shows whenever Win is held (Copilot+ PCs). |

### `tray`

The tray menu's switches. Tile writes these itself when you change one, and reads them at
startup, so you rarely need to edit them.

| Key | Default | Meaning |
| --- | --- | --- |
| `tile` | `true` | The window manager is on (the tray's Tile item). |
| `remap` | `false` | The key remapper is on (the tray's Remap item). Off until switched on. |
| `logging` | `false` | Log to the log file (the tray's Logging item). |

These are separate from `general.tiling` and `general.remap`, which switch the features off
entirely: a feature runs only when both its `general` setting and its tray switch are on.

### `cursor`

| Key | Default | Meaning |
| --- | --- | --- |
| `warp` | `true` | Whenever a keybind or dispatch moves focus, put the pointer on the focused window (the monitor's centre if it is empty), unless it is already over it. The pointer is then hidden, as typing hides it (needs `hide_on_key`), until the mouse moves. |
| `hide_on_key` | `true` | Hide the pointer while typing, until the mouse moves or clicks. Modifiers on their own and keybinds don't count as typing. |
| `drag` | `true` | Hold Win and drag a window with the left button, from anywhere on it. On release the window is re-tiled where it is dropped. |

### `animations`

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Ease windows into place. |
| `duration_ms` | `180` | Length of a move or resize. |
| `bezier` | `[0.0, 0.75, 0.15, 1.0]` | Cubic-bezier control points `x1, y1, x2, y2`. |

### Layout sections

| Section | Key | Default | Meaning |
| --- | --- | --- | --- |
| `dwindle` | `split_ratio` | `0.5` | Share of a split given to the existing window. |
| `master` | `mfact` | `0.5` | Width of the master area, as a share of the monitor. |
| `master` | `count` | `1` | Windows in the master area. |
| `master` | `new_is_master` | `false` | New windows become the master. |
| `scrolling` | `proportion` | `0.333` | Width of new columns, as a share of the monitor. |
| `scrolling` | `proportion_presets` | `[0.25, 0.333, 0.5, 1.0]` | Widths `cycleproportion` steps through. |
| `scrolling` | `center_focused` | `on_overflow` | When to centre the focused column: `always`, `on_overflow` (when it would otherwise be partly off screen) or `never`. |

### `launcher`

The app launcher, opened by the `launcher` dispatcher. See [launcher](launcher.md).

| Key | Default | Meaning |
| --- | --- | --- |
| `width` | `560` | Width in pixels at 100% scaling (scaled with the monitor). |
| `height` | `500` | Height in pixels at 100% scaling. |
| `search_position` | `top` | Where the search box sits: `top` or `bottom`. |
| `show_icons` | `true` | Show each app's icon. |
| `compact` | `false` | Shorter rows, without the app id under the name. |
| `max_results` | `50` | The most apps listed. |
| `track_usage` | `true` | Remember launches in `%LOCALAPPDATA%\Tile\launcher.json`. |
| `sort_by_usage` | `true` | List recent launches first, and let often launched apps win close matches. |
| `ignore_mouse` | `false` | The mouse doesn't select or launch. |
| `pinned` | `[]` | App names or ids listed first, in this order, before anything is typed. |
| `hide` | `[]` | Regexes (case-insensitive) on the app name or id; matching apps are never listed. |
| `font` | `Segoe UI` | Font family. |
| `font_size` | `11` | Font size in points. |
| `corners` | `round` | Window corners: `round`, `small` or `square` (Windows 11). |
| `opacity` | `1.0` | How opaque the whole window is, from `0.1` to `1.0`. |
| `background` | `"#1e1e2e"` | Window colour. |
| `border` | `"#89b4fa"` | The window's outline (Windows 11). |
| `foreground` | `"#cdd6f4"` | Text colour. |
| `dimmed` | `"#7f849c"` | The app id line and the placeholder. |
| `selected` | `"#89b4fa"` | The selected row. |
| `selected_foreground` | `"#1e1e2e"` | Text on the selected row. |
| `placeholder` | `""` | Shown in the empty search box; `{prefix}` becomes `command_prefix`. Empty shows `Search apps or equations, ! for commands` with the real prefix, leaving out the calculator or commands when they are off. |
| `calculator` | `true` | Show the value of a search that reads as a calculation first; Enter copies it. See [calculator](launcher.md#calculator). |
| `command_prefix` | `"!"` | Typing this first runs a command line or a Tile dispatcher instead of searching apps. `""` switches it off. See [commands](launcher.md#commands). |

### `binds`

Keybinds, as `<keys>: <dispatcher>`. They add to the [default binds](keybinds.md#default-keys):
a key that is already bound gets your dispatcher instead, and an empty value (`""` or `~`)
unbinds it. Set `general.default_binds: false` to start from nothing.

```yaml
binds:
  super+t: exec wtd.exe -w new          # replaces the default Win+T
  super+e: focusorexec code.cmd         # focus VS Code, or start it
  super+z: ~                            # unbind the default Win+Z
```

Launching, closing, locking, desktops, `reload`, `togglepause` and `exit` work even with tiling
off or paused; the rest only run while Tile is on (the tray's Tile item). See
[keybinds](keybinds.md) for the key names and every dispatcher.

### `rules`

Per-window overrides. See [window rules](window-rules.md).

### `remap`

| Key | Default | Meaning |
| --- | --- | --- |
| `modifier_guard` | `true` | A layer's lone Alt or Win press doesn't open a menu or Start. |
| `layers.<name>` | see below | keyd-style layers. See [key remapping](key-remapping.md). |

## The default config

This is the configuration Tile runs with when the file is empty or missing, so it can be copied
as it is. A test checks that this block lists every setting at its real default.

<!-- default-config:start -->
```yaml
---
general:
  tiling: true                  # manage windows; false leaves them alone
  default_binds: true           # start from the default binds; false leaves only yours
  remap: true                   # remap keys with the remap layers
  gaps_in: 10                   # px between neighbouring windows
  gaps_out: 10                  # px between windows and the monitor edge
  independent_workspaces: true  # each monitor has its own workspaces; false syncs them
  layout: master                # dwindle | master | monocle | scrolling | even
  borders: true                 # colour the window border to show focus (Windows 11)
  border_active: "#89b4fa"
  border_inactive: "#45475a"
  focus_new_windows: true
  decorations: false            # keep title bars on tiled windows
  override_win_l: true          # take over Win+L
  disable_click_to_do: true     # switch off Click to Do

# The tray's switches. Tile saves these when you change them in the tray.
tray:
  tile: true                    # window manager on
  remap: false                  # key remapper on (starts off)
  logging: false

cursor:
  warp: true                    # move the pointer onto the window a keybind focuses, then hide it
  hide_on_key: true             # typing hides the pointer until the mouse moves or clicks
  drag: true                    # hold Win and drag a window with the left button

animations:
  enabled: true
  duration_ms: 180
  bezier: [0.0, 0.75, 0.15, 1.0]   # cubic-bezier control points (x1, y1, x2, y2)

dwindle:
  split_ratio: 0.5              # share of a split given to the existing window

master:
  mfact: 0.5                    # width of the master area
  count: 1                      # windows in the master area
  new_is_master: false          # new windows become master

scrolling:
  proportion: 0.333             # width of new columns, as a share of the monitor
  proportion_presets: [0.25, 0.333, 0.5, 1.0]   # widths `cycleproportion` steps through
  center_focused: on_overflow   # always | on_overflow | never

remap:
  modifier_guard: true          # a layer's lone Alt or Win doesn't open a menu or Start
  layers: {}                    # none; see key-remapping.md

launcher:
  width: 560                    # px at 100% scaling
  height: 500
  search_position: top          # top | bottom
  show_icons: true
  compact: false                # shorter rows, no app id line
  max_results: 50
  track_usage: true             # remember launches
  sort_by_usage: true           # recent first, often used win close matches
  ignore_mouse: false
  pinned: []                    # names or ids listed first
  hide: []                      # regexes on name or id never listed
  font: Segoe UI
  font_size: 11                 # points
  corners: round                # round | small | square
  opacity: 1.0                  # 0.1 to 1.0
  background: "#1e1e2e"
  border: "#89b4fa"
  foreground: "#cdd6f4"
  dimmed: "#7f849c"             # app ids and the placeholder
  selected: "#89b4fa"
  selected_foreground: "#1e1e2e"
  placeholder: ""                # "" for "Search apps or equations, ! for commands"; {prefix} is the prefix
  calculator: true              # `12 * 3` shows = 36; Enter copies it
  command_prefix: "!"           # run a command line or dispatcher; "" for off

binds: {}                       # added to the default binds; see keybinds.md

rules:
  # Standard dialogs float.
  - class: "^#32770$"
    float: true
    ignore: false
  # Shell windows are left alone.
  - class: '^(Windows\.UI\.Core\.CoreWindow|XamlExplorerHostIslandWindow|Shell_TrayWnd)$'
    ignore: true
  # So are PowerToys and ShareX.
  - exe: '^(PowerToys.*|ShareX)\.exe$'
    ignore: true
```
<!-- default-config:end -->
