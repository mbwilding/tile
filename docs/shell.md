# Shell mode

Tile can be your Windows shell: no Explorer taskbar, desktop or Start menu, just your tiled
windows. File Explorer still works for browsing folders. It is optional and off by default.

What Explorer would otherwise provide is covered like this:

| Explorer's | In shell mode |
| --- | --- |
| Tray icons (Discord, Steam, ...) | Tile hosts the notification area; the [launcher](launcher.md#system-items-and-tray-icons) lists the icons and clicks them |
| Volume, network, Wi-Fi, Bluetooth, battery, clock | Launcher rows that show and change them |
| Start menu, search | The launcher |
| Sign out, restart, shut down | Launcher rows (restart, shut down and sign out ask for Enter twice) |
| Programs started at sign-in | Tile runs the `Run` and `RunOnce` keys and the Startup folders |
| File Explorer windows | Unchanged |

## Turning it on

```powershell
tilectl install          # the shell has to live somewhere that stays
```

Then tick **Shell** in the tray menu, run the `toggleshell` dispatcher, or set `shell: true`
under `tray` in your config. It takes effect at your next sign-in. Ticking it sets your user's
Winlogon `Shell` value to `tile.exe --shell`, which needs no admin rights and affects only your
account, and turns on "Launch folder windows in a separate process" so a folder window doesn't
bring Explorer's taskbar back. Unticking it puts Explorer back from the next sign-in.

The switch is the config's `tray.shell`, and Tile keeps the sign-in shell in step with it every
time it starts and whenever the file changes. So a config with `shell: true` is enough on a
first run, and a config set back to `false` restores Explorer.

The launcher is how you start everything in shell mode, so in shell mode it is always bound. If
you haven't bound the `launcher` dispatcher yourself, Tile binds it to `Win+R`, or to `Win+D` if
`Win+R` is taken. The tray icons are a row of their own at the top of the launcher, under its bar.

### Without signing out

The tray's **Shell** item (next to Tile and Remap, remembered in `tray.shell`) makes a running
Tile act as the shell whenever no taskbar is running. With it on, end
Explorer (`taskkill /f /im explorer.exe`) and Tile starts hosting the notification area within a
couple of seconds. Apps that were running add their tray icons again when Tile announces the new
taskbar. If Explorer's taskbar comes back, Tile hands the notification area back to it. This does not
run the sign-in programs, since Explorer already did.

## What Tile does as the shell

**Notification area.** Apps add their tray icons by sending a message to the window of class
`Shell_TrayWnd`. Tile creates that window when no taskbar exists and keeps the icons it is sent.
Opening one in the launcher passes your click on to the app the way Explorer does: Enter is a
left click and Shift+Enter (or the right mouse button) is a right click, so the app shows its
window or menu. The menu appears where the pointer is.

**Sign-in programs.** When Tile starts and no taskbar is running, it runs what Explorer would:
`RunOnce` (removing each entry as it runs), then `Run`, from `HKLM` and `HKCU`, then the shared
and your own Startup folders. Entries switched off in Task Manager are skipped, and so is Tile's
own `--launcher` entry, because the shell instance already does that job. It happens once per
sign-in session, so restarting Tile doesn't start everything twice. `shell.run_startup: false`
turns it off.

**Admin rights.** `tilectl install` runs Tile as administrator through a sign-in task. As the
shell, Windows also starts `tile.exe --shell` with your normal rights. That copy serves as the
helper that starts programs without admin rights and waits for the elevated Tile. If the sign-in
task never starts Tile within 30 seconds, the helper runs Tile itself, without admin rights, and
logs it to `launcher.log`.

## What you give up

- **No desktop or wallpaper.** Explorer paints both. While Tile hosts the notification area it
  covers each monitor with a plain black window at the very bottom of the stack (Windows keeps
  showing Explorer's last wallpaper otherwise), and removes them when it stops. Icons on the desktop are gone.
- **No Start menu, search, quick settings or notification centre.** The launcher covers starting
  apps and the volume, network, Bluetooth, battery and clock. Windows toast notifications and
  tray balloons are not shown.
- **Alt+Tab, Task View and snapping** are Explorer's; they may be missing or odd. Tile's
  workspaces replace most of that.
- **Some Store apps** that expect Explorer's shell experience host may not start.
- **Win+E** and the Win key shortcuts that Explorer handles stop working. Bind your own, for
  example `super+e: exec explorer.exe`.

Opening a folder with `explorer.exe C:\path` gives a window on its own when folder windows run
in a separate process, which the Shell switch sets up. `explorer.exe` with no path may
start the desktop and taskbar again, depending on the Windows build; if the taskbar appears,
end `explorer.exe` from Task Manager.

## Getting out of trouble

If Tile crashes or you quit it, you are left on a black screen. Press `Ctrl+Alt+Del`, open Task
Manager, choose Run new task and run `explorer.exe`. To go back for good, untick Shell in the
tray (or set `tray.shell: false` in the config), then sign out and in.

The shell is only as good as the config it loads. A config with errors keeps the last good one
(or the built-in defaults), and the default binds still work, so keep `general.default_binds` on
until you are happy with your own.
