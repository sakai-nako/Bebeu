# User preferences

User preferences (`preferences.yml`) are stored independently of the workspace, under the OS-standard user config directory:

| OS | Path |
|---|---|
| Windows | `%APPDATA%\local-game-editor\preferences.yml` |
| macOS | `~/Library/Application Support/local-game-editor/preferences.yml` |
| Linux | `~/.config/local-game-editor/preferences.yml` |

The directory name `local-game-editor` is the legacy package name kept for backward compatibility, so existing users do not need to migrate their settings.

Fields currently stored:

- Theme (`emerald` / `dark`, default `emerald`)
- View controls / zoom step for Level editing
- Undo/redo history capacity — held **independently for SpriteGroup / Animation / Level** (each defaults to 50 steps)
- Key bindings (editable from the `Edit Key Bindings` modal inside the editor)

If the file is missing or corrupt, it falls back to `Default::default()` (fail-soft). Old `preferences.yml` files missing newer fields are individually backfilled via `#[serde(default)]`.
