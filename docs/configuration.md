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
| `tile` | `false` | The window manager is on (the tray's Tile item). Off until switched on. |
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
  tile: false                   # window manager on (starts off)
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
