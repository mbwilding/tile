# Launcher

A search box for starting apps, like wofi or Noctalia's launcher: a window centred on the focused
monitor that lists everything Start lists and starts what you pick.

It opens on the monitor of the focused window, whatever that window is (tiled, floating or one
Tile leaves alone). With nothing focused, such as on an empty monitor, it opens on Tile's focused
monitor, or with tiling off, the monitor under the pointer.

Open it with the `launcher` dispatcher. It isn't bound by default; bind it in your config, for
example to `Win+R` (a bound `Win` key replaces Windows' own shortcut, here the Run dialog):

```yaml
binds:
  super+r: launcher
```

`tilectl dispatch launcher` opens it too. It works with tiling off or paused. Pressing the bind
again closes it.

This is not the `tile.exe --launcher` helper that starts programs with your normal rights; the
launcher uses that helper too, so apps it starts never run as administrator.

## What it lists

Every app in Start's All apps list (`shell:AppsFolder`): desktop programs and Store apps alike,
with their icons. The list is read again in the background each time the launcher opens, so new
installs show up. Hide entries you never want (uninstallers, help files) with `launcher.hide`,
regexes matched against the app's name or id.

Each row shows the app name and, unless `compact` is on, its id: the AppUserModelID of a Store
app or the path of a desktop program.

## Searching

Type to filter. Matching is fuzzy: the letters must appear in order, not next to each other, and
case doesn't matter. Matches are ranked as in Noctalia:

- A match on the app name always beats a match on its id.
- A name that starts with what you typed gets a bonus.
- Letters that start words (`vs` in **V**isual **S**tudio) and runs of consecutive letters score
  higher than scattered ones.
- With `sort_by_usage` on, apps you launch often win close matches, without beating a much
  better match.

Before anything is typed the list shows `pinned` apps first, in their order, then (with
`sort_by_usage`) the 20 most recently launched, newest first, then everything else by name.

## Calculator

Type a sum and its value is the first row: `12 * 3` shows `= 36`. Enter (or a click) copies the
value to the clipboard and closes the launcher. Apps that match the same text are listed under it.

A search counts as a calculation when it has a digit and is more than a bare number, so `7zip`
or `42` stay app searches. It understands:

- `+`, `-`, `*` (or `x` or `×`), `/` (or `÷`), `%` (remainder) and `^` (or `**`), with
  brackets; spaces don't matter, so `5x5` and `5 * 5` are the same
- numbers like `1.5`, `1_000`, `2.5e3` and hex `0x1f` (`0x` is always hex, never times zero)
- `pi`, `tau` and `e`
- `sqrt`, `cbrt`, `abs`, `ln`, `log` (base 10), `log2`, `exp`, `sin`, `cos`, `tan`, `asin`,
  `acos`, `atan` (radians), `floor`, `ceil` and `round`, as `sqrt(2)`

A sign binds looser than `^`, so `-2^2` is -4. Multiplication is never implied: write `2 * pi`,
not `2pi`. Set `calculator: false` to switch it off.

## Commands

Start the search with `launcher.command_prefix` (`!` by default) to run something other than an
app. What follows the prefix can be:

- **A Tile dispatcher**, as in a bind: `!workspace 3`, `!layout scrolling`, `!reload`. The
  dispatcher is the first row. Dispatchers that act on a window act on the one that had focus
  before the launcher opened.
- **A command line**, like the Run dialog: `!notepad notes.txt`, `!cmd`, `!\\server\share`, a
  path or a URL. It runs as `exec` does, through `start` and with your normal rights. A
  **Run command** row is always there, under any dispatcher row, so text that is also a
  dispatcher can still run as a command line.

`!` on its own lists every dispatcher. Typing part of a name (`!work`) lists the dispatchers
that start with it; picking one that takes an argument puts its name in the search box for you
to finish (`!workspace `), and picking one that doesn't runs it. Set `command_prefix: ""` to
switch commands off.

## Keys

| Key | Action |
| --- | --- |
| `Enter` | start the selected row (run a command, copy a sum) |
| `Esc` | close |
| `Up` / `Down`, `Ctrl+K` / `Ctrl+J`, `Ctrl+P` / `Ctrl+N` | select the previous / next row |
| `Tab` / `Shift+Tab` | next / previous row |
| `Page Up` / `Page Down` | move a page |
| `Left` / `Right`, `Ctrl+H` / `Ctrl+L` | move the caret a character |
| `Ctrl+Left` / `Ctrl+Right` | move the caret a word |
| `Home` / `End` | move the caret to the start / end |
| `Backspace` / `Delete` | delete the character before / after the caret |
| `Ctrl+Backspace` / `Ctrl+W` | delete the word before the caret |
| `Ctrl+U` | delete everything before the caret |

Keys you remap to the arrows (a `remap` layer) work the same as the arrows.

With the mouse, hovering selects, clicking starts and the wheel scrolls (unless `ignore_mouse` is
on). Clicking anywhere else, or switching to another window, closes it.

## Launch history

With `track_usage` on, each launch is counted in `%LOCALAPPDATA%\Tile\launcher.json`, which
holds the launch count per app and the 20 most recent. Delete the file to forget them.

## Appearance

Size, position of the search box, font, colours, corners and opacity are all in the
[`launcher` section](configuration.md#launcher) of the config. The font, corners and colours
follow the shared [`theme`](configuration.md#theme) unless set there. Sizes are in pixels at 100%
scaling and grow with the monitor's scaling. Changes apply the next time it opens.
