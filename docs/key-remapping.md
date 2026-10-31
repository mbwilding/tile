# Key remapping

Tile has a [keyd](https://github.com/rvaiya/keyd)-style remapper built in: layers, home-row mods
and tap/hold keys. Switch it on and off with `general.remap` or the tray's Key item (it starts
off). The remapper has its own keyboard hook in front of the keybinds, so binds see keys as
remapped.

`remap.layers` holds keyd's config as YAML: each entry is a keyd section, named like keyd's
section header (quoted when it has `:`), and maps keys to actions. Quote keys and actions that
start with punctuation, like `";"` or `"!"`. There are no layers by
default, so the remapper changes nothing until you add some.

```yaml
remap:
  layers:
    main:
      capslock: overload(nav, esc)    # tap for Escape, hold for the nav layer
      a: overloadt(control, a, 200)   # Control once held for 200ms
      rightalt: layer(nav)
    "nav:C":                          # keys nav doesn't bind get Control while it's held
      h: left
      l: right
    nav+shift:                        # while nav and shift are both active
      h: C-left
```

| Action | Effect |
| --- | --- |
| `<key>` / `C-left` / `!` | send a key, with modifiers or as a shifted character |
| `layer(<name>)` | the layer is active while the key is held |
| `overload(<name>, <key>)` | the layer while held, the key if tapped alone |
| `overloadt(<name>, <key>, <ms>)` | the layer once held for `ms`, the key if released before |
| `noop` | nothing |

keyd's semantics apply: layers stack, unbound keys fall through to `main`, a layer's modifiers
apply to the keys it doesn't bind, and keys typed while an `overloadt` key is deciding are
replayed in order once it's decided, so fast rolls type normally. Keys no layer touches are let
through untouched.

Keys are physical positions named after the US QWERTY layout (keyd's names: `a`, `;`, `/`,
`leftshift`, `capslock`, `esc`, `f1`…), not characters, and Windows' keyboard layout applies to
what Tile sends: with the Windows Dvorak layout, the key named `j` types `h`. Built-in layers:
`control`, `shift`, `alt`, `meta` and `altgr`.
Modifier letters: `C`, `S`, `A`, `M` (Win) and `G` (AltGr).

`modifier_guard` (on by default) stops a layer's lone Alt or Win press opening a menu or Start.

## Limits

Not supported yet: `oneshot`, `toggle`, `swap`, `macro`, `overloadt2`, `timeout` and keyd's other
actions, aliases and `include`. Per-keyboard `[ids]` can't be: a low-level keyboard hook can't
tell keyboards apart, so the layers apply to every keyboard.

## Emergency exit

Holding Backspace + Escape + Enter (physical keys) turns remapping off until the config is
reloaded, as in keyd.
