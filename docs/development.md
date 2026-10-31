# Development

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all
```

`cargo run` starts the daemon (the default member); pass `--workspace` to build or test
everything. See the README for running a build without installing it.

## Project layout

| Crate | Purpose |
| --- | --- |
| `tile-core` | Platform-independent state, layouts, dispatchers and easing (unit tested) |
| `tile-remap` | The remapping engine: keyd's layers and overloads as a pure state machine (tested) |
| `tile-config` | YAML schema, keybind parsing, window rules, remap layers, the tray switches and file watching |
| `tile-hook` | The low-level keyboard and mouse hooks, `SendInput` and other raw calls: the only `unsafe` |
| `tile-platform` | Win32: window events, placement and animation, the keybind hook, the Win+drag mouse hook, tray, virtual desktops |
| `tile-ipc` | Named-pipe JSON protocol |
| `tile` | The daemon (`tile.exe`) |
| `tilectl` | The command-line tool (`tilectl.exe`), including install/uninstall |

All `unsafe` lives in `tile-hook`, each block with a `// SAFETY:` comment; every other crate has
`unsafe_code = "forbid"`. The rest of Win32 goes through safe wrappers:
[`winsafe`](https://crates.io/crates/winsafe),
[`wineventhook`](https://crates.io/crates/wineventhook),
[`uiautomation`](https://crates.io/crates/uiautomation),
[`tray-icon`](https://crates.io/crates/tray-icon) and
[`winvd`](https://crates.io/crates/winvd).

`tile-core` holds the whole model: `State` owns monitors, workspaces and window membership, and
every change returns a list of effects (place, show, hide, focus, close, exec) for the daemon to
apply. That keeps the layouts and dispatchers testable without Windows.

## Releasing

Every push to `main` that changes more than docs runs the `Release` workflow, which tests, builds
and publishes a GitHub release holding `tile.zip`, which has `tile.exe` and `tilectl.exe`. No
tag is pushed: the version is the major and minor from `Cargo.toml`, with the workflow run number
as the patch (`0.1.7`), and the release creates its own tag. To release by hand, run it from the
Actions tab or with `gh workflow run release.yml`. Bump the major or minor in `Cargo.toml` for a
bigger release.

## Documentation

Everything is documented, and the docs change with the code:

- Doc comments on every public item.
- `docs/configuration.md` lists each setting and a copy-paste default config. A test
  (`crates/tile-config/tests/docs.rs`) fails when that block and the real defaults disagree, so
  a new setting means updating the block.
- Keys and dispatchers go in `docs/keybinds.md`, layouts in `docs/layouts.md`, and so on. The
  README stays an overview and quickstart, linking to these.

## Not yet

- `Win` + right-drag to resize a window in the direction the mouse moves, pushing the
  neighbouring tiles along.
