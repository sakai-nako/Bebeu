# Key bindings

Default keyboard shortcuts inside the editor. They can be remapped individually from the `Edit Key Bindings` modal, and the new bindings are persisted under the `key_bindings` section of `preferences.yml`.

## Common

| Key | Action |
|---|---|
| `Ctrl + S` | Save |
| `Ctrl + Z` | Undo |
| `Ctrl + Shift + Z` | Redo |

Undo / Redo applies to the currently active Editor.

## SpriteGroup Editor

| Key | Action |
|---|---|
| `Ctrl + ←` / `Ctrl + →` | Select previous / next Sprite |
| `Ctrl + Home` / `Ctrl + End` | Select first / last Sprite |
| `Ctrl + Shift + ←` / `Ctrl + Shift + →` | Move the selected Sprite earlier / later |
| `Shift + ↑` / `Shift + ↓` | Move pivot down / up |
| `Shift + ←` / `Shift + →` | Move pivot right / left |

(Pivot move directions are intentionally inverted relative to the arrow direction in the defaults. Remap them from the modal if you find it counter-intuitive.)

## Animation Editor

| Key | Action |
|---|---|
| `Ctrl + ←` / `Ctrl + →` | Select previous / next Frame |
| `Ctrl + Home` / `Ctrl + End` | Select first / last Frame |
| `Space` | Play / pause the Animation |
| `Shift + Space` | Stop playback (rewind to the first frame) |
