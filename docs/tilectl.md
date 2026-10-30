# tilectl

The command-line tool for a running Tile.

```powershell
tilectl dispatch workspace 3      # run any dispatcher (see keybinds.md)
tilectl query windows             # monitors | workspaces | windows | activewindow; --json for JSON
tilectl subscribe                 # JSON lines on workspace, active window and layout changes
tilectl reload                    # reload the config
tilectl check [file]              # check a config file without applying it
tilectl config                    # print the config file's path
tilectl install / uninstall       # see the README
```

`tilectl query windows` is the way to find the `exe`, `class` and `title` to write
[window rules](window-rules.md) against.

## The pipe

`tilectl` talks to Tile over the named pipe `\\.\pipe\tile.sock`: one JSON request per line, JSON
responses back. Scripts can use it directly. Only your account (and administrators) can connect,
including from a normal terminal when Tile runs as administrator. The protocol is defined in the
`tile-ipc` crate.
