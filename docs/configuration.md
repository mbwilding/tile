# Configuration

Tile reads `%APPDATA%\Tile\config.toml` (`tilectl config` prints the path). Tile runs on its
built-in defaults, so the file only needs the settings you want to change. On first run it
creates the file holding just `[tray]`, the tray's switches, which Tile saves there whenever you
change them. Add a `[section]` and key to change anything else; the [default
config](#the-default-config) below lists them all to copy from. Saved changes apply immediately,
and `tilectl check [file]` reports what is wrong with a config. A bad config is not applied; the
previous one is kept.

These tables **replace** the built-in set entirely when you write them, so copy the defaults from
the end of this page as a starting point:

- `[binds]`
- `[hotkeys]`
- `[[rules]]`
- `[remap.layers]`

Every other table merges: a key you leave out keeps its default. Unknown keys are an error.

## Reference

### `[general]`

| Key | Default | Meaning |
| --- | --- | --- |
| `tiling` | `true` | Manage windows. `false` leaves them alone and gives Win+L back; turning it on again manages the open windows. |
| `remap` | `true` | Run the key remapper with the `[remap]` layers. |
| `gaps_in` | `10` | Pixels between neighbouring tiled windows. |
| `gaps_out` | `10` | Pixels between tiled windows and the monitor edge. |
| `independent_workspaces` | `true` | Every monitor has workspaces of its own, so `workspace 2` is workspace 2 on the monitor you are on and never moves focus to another monitor. `false` syncs the monitors: switching workspace switches them all together. See [layouts](layouts.md#workspaces-and-monitors). |
| `layout` | `"master"` | Layout of new workspaces: `dwindle`, `master`, `monocle`, `scrolling` or `even`. New workspaces on a portrait monitor start with `even` instead. See [layouts](layouts.md). |
| `borders` | `true` | Colour the window border to show focus (Windows 11). |
| `border_active` | `"#89b4fa"` | Border colour of the focused window. |
| `border_inactive` | `"#45475a"` | Border colour of every other tiled window. |
| `focus_new_windows` | `true` | Focus a window when it opens. |
| `decorations` | `false` | Keep title bars on tiled windows. Floating windows always keep theirs; a rule can override this per app. |
| `override_win_l` | `true` | Take over Win+L so it can be bound. See [Windows integration](windows-integration.md). |
| `disable_click_to_do` | `true` | Switch off Click to Do, whose glow shows whenever Win is held (Copilot+ PCs). |

### `[tray]`

The tray menu's switches. Tile writes these itself when you change one, and reads them at
startup, so you rarely need to edit them.

| Key | Default | Meaning |
| --- | --- | --- |
| `tile` | `false` | The window manager is on (the tray's Tile item). Off until switched on. |
| `key` | `false` | The key remapper is on (the tray's Key item). Off until switched on. |
| `hotkey` | `true` | The `[hotkeys]` are on (the tray's Hotkey item). |
| `logging` | `false` | Log to the log file (the tray's Logging item). |

These are separate from `general.tiling` and `general.remap`, which switch the features off
entirely: a feature runs only when both its `general` setting and its tray switch are on.

### `[cursor]`

| Key | Default | Meaning |
| --- | --- | --- |
| `warp` | `true` | Whenever a keybind or dispatch moves focus, put the pointer on the focused window (the monitor's centre if it is empty), unless it is already over it. The pointer is then hidden, as typing hides it (needs `hide_on_key`), until the mouse moves. |
| `hide_on_key` | `true` | Hide the pointer while typing, until the mouse moves or clicks. Modifiers on their own and keybinds don't count as typing. |
| `drag` | `true` | Hold Win and drag a window with the left button, from anywhere on it. On release the window is re-tiled where it is dropped. |

### `[animations]`

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Ease windows into place. |
| `duration_ms` | `180` | Length of a move or resize. |
| `bezier` | `[0.0, 0.75, 0.15, 1.0]` | Cubic-bezier control points `x1, y1, x2, y2`. |

### Layout tables

| Table | Key | Default | Meaning |
| --- | --- | --- | --- |
| `[dwindle]` | `split_ratio` | `0.5` | Share of a split given to the existing window. |
| `[master]` | `mfact` | `0.5` | Width of the master area, as a share of the monitor. |
| `[master]` | `count` | `1` | Windows in the master area. |
| `[master]` | `new_is_master` | `false` | New windows become the master. |
| `[scrolling]` | `proportion` | `0.333` | Width of new columns, as a share of the monitor. |
| `[scrolling]` | `proportion_presets` | `[0.25, 0.333, 0.5, 1.0]` | Widths `cycleproportion` steps through. |
| `[scrolling]` | `center_focused` | `"on_overflow"` | When to centre the focused column: `always`, `on_overflow` (when it would otherwise be partly off screen) or `never`. |

### `[binds]`

Window manager keybinds, as `"<keys>" = "<dispatcher>"`. They only run while Tile is on (the tray's
Tile item). See [keybinds](keybinds.md) for the key names and every dispatcher.

### `[hotkeys]`

Launch, focus and close keys. They work with tiling off, and are switched on and off together by
the tray's Hotkey item (which starts on). An entry is one of:

| Form | Meaning |
| --- | --- |
| `"super+b" = "chrome.exe"` | Focus a window of that program, or start it. The program is the command's file name. |
| `"super+t" = { run = "wt.exe", focus = false }` | Always start a new instance, never focus an existing one. |
| `"super+e" = { run = "code.cmd", exe = "Code.exe" }` | Start `run`, but focus a window of the process `exe` first if there is one. Use it when the process has another name than the command. |
| `"super+t" = { run = ["wtd.exe", "wt.exe"], focus = false }` | Run the first command whose program is installed, here Windows Terminal Dev, else Windows Terminal. |
| `"super+q" = { close = true }` | Close the focused window, like Alt+F4. |

`run` is a command line, so it can take arguments: `{ run = "chrome.exe --new-window", focus =
false }` opens a new Chrome window every time. A program counts as installed when it is a path that
exists, is on `PATH` (app execution aliases like `wt.exe` included) or is registered under App
Paths (like `chrome.exe`). If none is, the last command runs anyway. Without `exe`, the window to
focus is the program of the command that was picked.

Pressing a launch key again moves on to the next window of that program on the current virtual
desktop. A key bound in both `[binds]` and `[hotkeys]` runs the `[binds]` entry.

### `[[rules]]`

Per-window overrides. See [window rules](window-rules.md).

### `[remap]`

| Key | Default | Meaning |
| --- | --- | --- |
| `modifier_guard` | `true` | A layer's lone Alt or Win press doesn't open a menu or Start. |
| `layers.<name>` | see below | keyd-style layers. See [key remapping](key-remapping.md). |

## The default config

This is the configuration Tile runs with when the file is empty or missing, so it can be copied
as it is. A test checks that this block lists every setting at its real default.

<!-- default-config:start -->
```toml
[general]
tiling = true                 # manage windows; false leaves them alone
remap = true                  # remap keys with the [remap] layers
gaps_in = 10                  # px between neighbouring windows
gaps_out = 10                 # px between windows and the monitor edge
independent_workspaces = true # each monitor has its own workspaces; false syncs them
layout = "master"             # dwindle | master | monocle | scrolling | even
borders = true                # colour the window border to show focus (Windows 11)
border_active = "#89b4fa"
border_inactive = "#45475a"
focus_new_windows = true
decorations = false           # keep title bars on tiled windows
override_win_l = true         # take over Win+L
disable_click_to_do = true    # switch off Click to Do

# The tray's switches. Tile saves these when you change them in the tray.
[tray]
tile = false                  # window manager on (starts off)
key = false                   # key remapper on (starts off)
hotkey = true                 # the [hotkeys] below
logging = false

[cursor]
warp = true                   # move the pointer onto the window a keybind focuses, then hide it
hide_on_key = true            # typing hides the pointer until the mouse moves or clicks
drag = true                   # hold Win and drag a window with the left button

[animations]
enabled = true
duration_ms = 180
bezier = [0.0, 0.75, 0.15, 1.0]   # cubic-bezier control points (x1, y1, x2, y2)

[dwindle]
split_ratio = 0.5             # share of a split given to the existing window

[master]
mfact = 0.5                   # width of the master area
count = 1                     # windows in the master area
new_is_master = false         # new windows become master

[scrolling]
proportion = 0.333            # width of new columns, as a share of the monitor
proportion_presets = [0.25, 0.333, 0.5, 1.0]     # widths `cycleproportion` steps through
center_focused = "on_overflow"                  # always | on_overflow | never

# Launch, focus and close keys. Work with tiling off; the tray's Hotkey item switches them.
[hotkeys]
"super+b" = { run = "chrome.exe --new-window", focus = false }                  # a new window each time
"super+t" = { run = ["wtd.exe -w new", "wt.exe -w new"], focus = false }   # Terminal Dev, else Terminal
"super+q" = { close = true }

[remap]
modifier_guard = true         # a layer's lone Alt or Win doesn't open a menu or Start

[remap.layers.main]
"/" = "overloadt(alt, /, 200)"
";" = "overloadt(control, ;, 200)"
a = "overloadt(control, a, 200)"
backspace = "noop"
capslock = "overload(capslock, backspace)"
f = "overloadt(numbers, f, 200)"
l = "overloadt(meta, l, 200)"
leftshift = "esc"
m = "overloadt(fkeys, m, 200)"
s = "overloadt(meta, s, 200)"
space = "overload(shift, space)"
z = "overloadt(alt, z, 200)"

[remap.layers."capslock:C"]
c = "down"
j = "left"
p = "right"
v = "up"

[remap.layers."capslock+shift"]
c = "C-down"
j = "C-left"
p = "C-right"
v = "C-up"

[remap.layers.fkeys]
a = "f5"
c = "f11"
d = "f7"
e = "f3"
f = "f8"
q = "f1"
r = "f4"
s = "f6"
v = "f12"
w = "f2"
x = "f10"
z = "f9"

[remap.layers.numbers]
"," = "2"
"." = "3"
";" = "0"
i = "8"
j = "4"
k = "5"
l = "6"
m = "1"
o = "9"
u = "7"

[binds]
"super+ctrl+h" = "workspace 1"
"super+ctrl+t" = "workspace 2"
"super+ctrl+n" = "workspace 3"
"super+ctrl+s" = "workspace 4"
"super+ctrl+minus" = "workspace 5"
"super+ctrl+shift+h" = "movetoworkspace 1"
"super+ctrl+shift+t" = "movetoworkspace 2"
"super+ctrl+shift+n" = "movetoworkspace 3"
"super+ctrl+shift+s" = "movetoworkspace 4"
"super+ctrl+shift+minus" = "movetoworkspace 5"
"super+6" = "workspace 6"
"super+7" = "workspace 7"
"super+shift+6" = "movetoworkspace 6"
"super+shift+7" = "movetoworkspace 7"
"super+left" = "movefocus l"
"super+right" = "movefocus r"
"super+up" = "movefocus u"
"super+down" = "movefocus d"
"super+h" = "movefocus l"
"super+j" = "movefocus d"
"super+k" = "movefocus u"
"super+l" = "movefocus r"                 # needs override_win_l (the default)
"super+shift+left" = "movewindow l"
"super+shift+right" = "movewindow r"
"super+shift+up" = "movewindow u"
"super+shift+down" = "movewindow d"
"super+shift+h" = "movewindow l"
"super+shift+j" = "movewindow d"
"super+shift+k" = "movewindow u"
"super+shift+l" = "movewindow r"
"super+alt+left" = "resizeactive -40 0"
"super+alt+right" = "resizeactive 40 0"
"super+alt+up" = "resizeactive 0 -40"
"super+alt+down" = "resizeactive 0 40"
"super+alt+h" = "resizeactive -40 0"
"super+alt+j" = "resizeactive 0 40"
"super+alt+k" = "resizeactive 0 -40"
"super+alt+l" = "resizeactive 40 0"
"super+ctrl+shift+left" = "movetodesktop prev"
"super+ctrl+shift+right" = "movetodesktop next"
"super+ctrl+p" = "pin"
"super+comma" = "focusmonitor l"
"super+period" = "focusmonitor r"
"super+enter" = "swapwithmaster"
"super+f" = "togglefloating"
"super+o" = "fullscreen"
"super+space" = "cyclelayout"
"super+n" = "cyclenext"
"super+z" = "lock"
"super+s" = "togglesplit"
"super+r" = "cycleproportion"
"super+c" = "centercolumn"
"super+bracketleft" = "consumewindow l"
"super+bracketright" = "consumewindow r"
"super+backslash" = "expelwindow"
"super+shift+r" = "reload"
"super+shift+p" = "togglepause"
"super+shift+e" = "exit"

# Standard dialogs float.
[[rules]]
class = "^#32770$"
float = true
ignore = false

# Shell windows are left alone.
[[rules]]
class = '^(Windows\.UI\.Core\.CoreWindow|XamlExplorerHostIslandWindow|Shell_TrayWnd)$'
ignore = true

# So are PowerToys and ShareX.
[[rules]]
exe = '^(PowerToys.*|ShareX)\.exe$'
ignore = true
```
<!-- default-config:end -->
