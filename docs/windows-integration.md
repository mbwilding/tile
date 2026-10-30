# Windows integration

How Tile fits in with Windows, and what it changes.

## Files

| Path | Contents |
| --- | --- |
| `%APPDATA%\Tile\config.toml` | [Config](configuration.md), created on first run with only the tray switches |
| `%LOCALAPPDATA%\Tile\logs` | Daily log files, while logging is on |
| `%LOCALAPPDATA%\Tile\restore.json` | What to undo if Tile dies, while it runs |
| `C:\Program Files\Tile` | The installed programs |

## The tray

Tile, Key and Hotkey switch the window manager, the key remapper and the `[hotkeys]` on or off on
their own, and are remembered across restarts in the config's `[tray]` table. Tile and Key start off; Hotkey starts on. The tray
also reloads and opens the config, switches logging on or off (nothing is logged until then, or
until `--log` is passed: `tile.exe --log tile=debug` logs every keybind it handles), opens the
logs and quits.

Pausing Tile hands every window back as it was: shown, with its title bar and border. Resuming
starts again from the windows that are open.

## Virtual desktops

Windows 11 24H2 and later. Every native virtual desktop has its own workspaces and layouts. Task
View, `Win+Ctrl+Left/Right` and "show this window on all desktops" keep working.

## Win+L

Windows handles Win+L before any keyboard hook sees it. With `override_win_l = true`, the default,
Tile sets the per-user `DisableLockWorkstation` policy while it runs, so Win+L can be bound (to
`movefocus r` by default) and the `lock` dispatcher locks instead. The policy is removed when Tile
stops, even after a crash or kill. If that policy is admin-only on your machine and Tile doesn't
run as administrator, it asks once through UAC to let your account manage that single registry
key. With `override_win_l = false`, Win+L locks as usual and binds to it are skipped.

## Click to Do

On Copilot+ PCs, holding Win shows Click to Do's glow around the pointer. Tile switches it off
once and leaves it off, because Windows shows a notice every time it is switched. To get it back,
set `disable_click_to_do = false` and turn it on in Settings, Privacy & security, Click to Do.

## Admin windows

A program running without admin rights can't move admin windows or see keys pressed in them. The
installed Tile runs as administrator, so it manages them like any other window. A Tile started by
hand without admin rights (e.g. `cargo run`) leaves them alone, and its keybinds and remapping
don't work while one has focus. Programs Tile starts (`exec` binds and `[hotkeys]`) go through a
launcher running with your normal rights, so they don't run as administrator.

## Secure desktops

No program sees keys on the lock screen, UAC prompts or Ctrl+Alt+Del. The remapper resets on lock
and unlock; after a UAC prompt, a layer key released on the prompt may need pressing again. If
Tile is killed (not quit), a modifier the remapper was holding stays down until you press and
release it.

## Minimum sizes

Apps that refuse a tile smaller than their minimum size are placed with `SWP_NOSENDCHANGING`, so
they fit their tile anyway.

## Undoing changes

Tile hides windows on inactive workspaces, removes title bars and sets the Win+L policy. It
records these in `restore.json` and undoes them on exit, pause, turning tiling off and on panic.
A watchdog process (`tile.exe --watchdog <pid>`) undoes them if Tile is killed, and the next
start cleans up anything left over. Held keys are released on exit, pause, reload, and when the
session locks or unlocks.
