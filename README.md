# Tile

A dynamic tiling window manager for Windows, with a [keyd](https://github.com/rvaiya/keyd)-style
key remapper built in (layers, home-row mods, tap/hold keys). Either side can be switched off.
Written in Rust, with all `unsafe` in one small crate.

Keybinds are handled in-process by a low-level keyboard hook, including `Win` combos (the Start
menu is suppressed), so no separate hotkey program is needed.

## Features

- Five layouts, chosen per workspace: **dwindle**, **master/stack**, **monocle**,
  **scrolling** (an endless strip of columns) and **even** (equal shares, used automatically on
  portrait monitors)
- Numbered workspaces, independent per monitor (or synced, if you prefer); floating and fullscreen windows
- Native virtual desktops (Windows 11 24H2+): every desktop has its own workspaces and layouts
- Mouse: hold `Win` and drag a window; drag a tile onto another to swap them, or its edge to
  resize the split
- Binds that start or focus a program (`Win+B` a new Chrome window, `Win+T` a new Windows
  Terminal window), and `Win+Q` to close, working even with tiling off
- Coloured focus borders (Windows 11) and eased move/resize animations
- Optional title bar removal for tiled windows, with per-app overrides
- Key remapping in keyd's terms: layers, modifier layers, `overload()`, `overloadt()` and key
  sequences
- YAML config, applied live on save, with window rules
- `tilectl`: dispatch commands, query state as JSON, subscribe to events, check the config,
  install/uninstall
- Tray icon with switches for the window manager (Tile) and the key remapper (Remap)
- Everything Tile changes is undone when it stops, including on a crash or a hard kill

## Quickstart

```powershell
cargo build --release --workspace    # builds tile.exe and tilectl.exe
.\target\release\tilectl install     # install and start Tile (one UAC prompt)
```

Tile starts tiling straight away. Tick **Remap** in the tray icon to remap keys, and untick
**Tile** (or press `Win+Shift+P`) to pause tiling; launch and close keys work either way. Try:

| Keys | Action |
| --- | --- |
| `Win+Arrows` / `Win+H/J/K/L` | move focus, across monitors too |
| `Win+Shift+Arrows` / `Win+Shift+H/J/K/L` | move the window |
| `Win+1`…`Win+9` | switch workspace |
| `Win+Space` | next layout |
| `Win+B` / `Win+T` | Chrome / Windows Terminal |
| `Win+Q` | close the window |
| `Win+Shift+E` | exit |

All the keys are in [Keybinds](docs/keybinds.md).

`tilectl install` sets Tile up to run as administrator, so it can manage admin windows too,
without a UAC prompt at sign-in:

- It copies both programs to `C:\Program Files\Tile`, where only admins can change them.
- A scheduled task starts Tile elevated when you sign in. It runs on battery, is never stopped
  for running too long and runs at normal priority.
- A launcher (`tile.exe --launcher`) starts with your normal rights from your `Run` key. Programs
  Tile starts, from `exec` binds or the tray, go through it, so they don't run as
  administrator.
- `C:\Program Files\Tile` goes on your user `PATH`, for `tilectl` in terminals started after your
  next sign-in.

It replaces a running Tile and asks for admin rights once. It also stops and removes the
standalone Key remapper (`C:\Program Files\Key`, its sign-in task and `PATH` entry) if that is
installed, since Tile remaps keys itself now; Key's config in `%APPDATA%\Key` is left alone. To
update after a rebuild, run `tilectl install` again from the checkout: the installed `tilectl`
installs the build in `.\target\release`. Running programs never block an update, since the old
files are moved aside and deleted on the next install. `tilectl uninstall` stops Tile and undoes
all of that, keeping your config and logs.

To try a build without installing:

```powershell
cargo run --release                  # runs tile.exe
.\target\release\tile.exe -c my.yml  # with another config file
```

Only one Tile runs at a time. Release builds have no console, so stop one with `Win+Shift+E`, the
tray icon or `tilectl dispatch exit`; Ctrl-C in the terminal only stops `cargo`. Closing the
terminal can leave windows hidden and the pointer blank until Tile next starts (see
[Windows integration](docs/windows-integration.md#undoing-changes)).

## Configuration

Tile runs on its built-in defaults. On first run it creates `%APPDATA%\Tile\config.yml` holding
only the tray's switches (Tile, Remap, logging), which it saves there as you change them.
Add a section and key to change anything else; saved changes apply immediately. [Configuration](docs/configuration.md) has every setting and the whole default
config to copy and paste, and [`examples/`](examples) has complete configs to start from.

## Documentation

| Page | Contents |
| --- | --- |
| [Configuration](docs/configuration.md) | Every setting, and the default config to copy |
| [Keybinds](docs/keybinds.md) | Default keys, key names, every dispatcher |
| [Layouts](docs/layouts.md) | The five layouts, portrait and multi-monitor behaviour |
| [Window rules](docs/window-rules.md) | Float, ignore or place windows by program, class or title |
| [Key remapping](docs/key-remapping.md) | keyd-style layers, overloads and their limits |
| [tilectl](docs/tilectl.md) | The command-line tool and the IPC pipe |
| [Windows integration](docs/windows-integration.md) | Tray, files, Win+L, admin windows, what gets undone |
| [Development](docs/development.md) | Crates, building, testing and the documentation rules |
