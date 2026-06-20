# ユーザー設定

ユーザー設定 (`preferences.yml`) は workspace dir とは独立に、OS 標準のユーザー設定ディレクトリ配下に保存される:

| OS | パス |
|---|---|
| Windows | `%APPDATA%\local-game-editor\preferences.yml` |
| macOS | `~/Library/Application Support/local-game-editor/preferences.yml` |
| Linux | `~/.config/local-game-editor/preferences.yml` |

ディレクトリ名 `local-game-editor` はパッケージ名変更前の旧名を保持しており、既存ユーザーの設定移行を避けるために据え置いている。

現状保持される項目:

- テーマ (`emerald` / `dark`、既定は `dark`)
- Level 編集の view controls / zoom step
- Level 編集の undo/redo 履歴 capacity
- key bindings (エディタ内の `Edit Key Bindings` モーダルから編集可)

ファイルが無いか壊れている場合は `Default::default()` でフォールバックする (fail-soft)。古い preferences.yml に新フィールドが欠けていても、`#[serde(default)]` で個別に補完される。
