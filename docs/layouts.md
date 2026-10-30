# Layouts

Each workspace has its own layout. Set the default for new workspaces with `general.layout`,
change the current one with the `layout <name>` and `cyclelayout` dispatchers (`Win+Space`), and
the windows keep their order when you switch. Options are in
[configuration](configuration.md#layout-tables).

| Layout | Description |
| --- | --- |
| `dwindle` | A binary tree. Each new window splits the focused one along its longer side. `togglesplit` flips a split, and dragging a tile's edge changes its ratio. |
| `master` | `master.count` windows in a column on the left (`mfact` of the width), the rest stacked on the right. `swapwithmaster` promotes a window. |
| `monocle` | Every window fills the workspace. Move focus steps through them. |
| `scrolling` | An endless horizontal strip of columns, scrolled to keep the focus in view. Columns can hold several windows (`consumewindow`, `expelwindow`), and off-screen columns are hidden. |
| `even` | Every window gets an equal share: side by side on a landscape monitor, stacked as equal rows on a portrait one. Resizing does nothing, since every share is equal. |

## Portrait monitors

A new workspace on a portrait monitor starts with `even`, whatever `general.layout` says.
Rotating a monitor switches its workspace to `even` (portrait) or back to the default layout
(landscape). Only workspaces that were on `even` or are now on a portrait monitor change; a
workspace you set to another layout stays on it until the monitor is rotated.

## Several monitors

Each monitor shows one workspace, and a workspace belongs to a monitor. Windows already open when
Tile starts are tiled on the monitor they are on, taken left to right then top to bottom so they
land in tiles near where they were, and new windows open on the monitor under the
mouse. `movefocus` and `movewindow` carry on to the next monitor at the edge of the current one,
landing on the window nearest the one you left.
