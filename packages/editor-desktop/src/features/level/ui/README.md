# features/level/ui — Level の CRUD と編集フォーム

Level 集約に対するユーザー操作 (作成 / 改名 / 削除 / 各プロパティの inline 編集 / OpponentTrigger 編集 / Save・Undo・Redo) を実装する feature slice。`features/character/ui` と同じ「1 アクション = 1 ファイル」方針と同じ 3 + 1 の UI パターンに従うので、パターンの基礎は [`features/character/ui/README.md`](../../character/ui/README.md) を参照し、本 README は **Level 固有の差分**に絞る。

## ファイル構成

facade は `level/ui.rs`。

```
create_level.rs         CreateLevelButton        — Pattern A (Modal): 新規作成 + base 画像 import
rename_level.rs         RenameLevelButton        — Pattern A (Modal): 改名
delete_level.rs         DeleteLevelButton        — Pattern C (Confirm): 削除 → /levels へ replace
edit_base_inline.rs     EditBaseInline           — single-action: base 画像差し替え + save
edit_camera_start.rs    EditCameraStart          — Pattern D (Inline): camera (X, Y)
edit_player_spawn.rs    EditPlayerSpawn          — Pattern D (Inline): player spawn (X, Z)
edit_player_respawn_y.rs EditPlayerRespawnY      — Pattern D (Inline): respawn Y
edit_gravity_scale.rs   EditGravityScale         — Pattern D (Inline): gravity_scale
edit_level.rs           LevelEditorActions       — Pattern D (Actions): Save / Cancel / Undo / Redo
opponent_triggers.rs    (facade)
opponent_triggers/
  ├─ triggers_section.rs OpponentTriggersSection — 一覧 + 「追加」
  ├─ trigger_row.rs      TriggerRow              — Pattern D (Inline): 個別 trigger 編集
  └─ delete_trigger.rs   DeleteTriggerButton     — 行削除
```

## UI パターンの Level 固有点

### Pattern A (Modal): `CreateLevelButton` / `RenameLevelButton`

character の Create/Rename と同型 (事前 `exists()` チェック + 事後 `create()` 失敗で TOCTOU を二重に防ぐ)。**Level 固有**は作成時に base 画像 import を伴うこと → 下記「base 画像 import」。

### Pattern C (Confirm): `DeleteLevelButton`

確認ダイアログ → `repo.delete()` → `refresh.bump()`。削除後は詳細ページが無効になるので **`nav.replace("/levels")`** で一覧へ戻す (character の削除と同じ離脱処理)。

### Pattern D (Inline): `EditCameraStart` / `EditPlayerSpawn` / `EditPlayerRespawnY` / `EditGravityScale`

表示 ↔ 編集モードを `editing: Signal<bool>` でトグルし、入力は **String の Signal** で保持 (`"400"` を編集途中の `"4"` で 4 に丸めない)。Apply で `parse` → 値が変わっていれば `history.record()` してから `draft.set(...)` する。**disk には書かない** — 永続化は親 `LevelEditorActions` の Save に集約する。これらは Canvas のドラッグ (`widgets/level/ui`) と同じ `draft` / `history` を共有するので、Canvas 操作と数値入力が双方向に反映される。

### single-action: `EditBaseInline`

確認なしの即時上書き。画像ピッカー → `repo.import_base_image` → `Level.base` 更新 → `repo.save` → `image_cache_buster` を bump して URL を再ロード (ADR-0015)。

## Pattern D の保存ライフサイクル (`LevelEditorActions`)

```
LevelDetail (draft: Signal<Level>, history: UseHistory<Level> を所有)
├── LevelCanvas        ← draft を読み書き (ドラッグ編集)
├── LevelInspector     ← 各 inline 編集が draft を読み書き
└── LevelEditorActions ← Save / Cancel / Undo / Redo
```

`LevelEditorActions` は `original` を `baseline: Signal<Level>` に複製して持ち、`is_dirty = draft() != *baseline.read()` で未保存を判定する。

- `use_effect` で dirty を `NavigationGuard` に同期する (breadcrumb / 左 rail / Cancel のすべての離脱経路で確認ダイアログが出る、ADR-0008)。`use_drop` で unmount 時に blocked を解除 (異常系で confirm が残らないように)。
- **Save**: `repo.save(draft)` 成功で `baseline.set(draft)` + `refresh.bump()`。baseline を別に持つのは、保存直後に `refresh.bump()` → `original` prop 更新のラグで dirty バッジが点滅するのを防ぐため。
- **Ctrl+S / Undo / Redo**: `use_keyboard_action(Action::{Save, Undo, Redo})` で発火 (ADR-0009)。`UseHistory` は `Copy` なので各クロージャに別コピーを渡し、`let mut h = history; h.undo();` する。
- **Cancel**: `guard.try_navigate(&nav, "/levels")` (dirty なら global confirm を挟む)。

## OpponentTrigger 編集

`OpponentTriggersSection` が `draft.opponent_triggers` を行 (`TriggerRow`) で並べる。

- **追加**: `OpponentTrigger::default()` を push して `history.record()`。新規行は `character_name == ""` なので `TriggerRow` が自動で編集モードに入る。
- **`TriggerRow`** (Pattern D Inline): `character_name` の select + `trigger_x` / `spawn_x` / `spawn_y` / `spawn_z` の number 入力。Apply で 4 値を parse → `draft.opponent_triggers[index]` を差し替え → `history.record()`。`trigger_x` / `spawn_x` / `spawn_z` は Canvas でもドラッグできる (`widgets/level/ui` の `TriggerLine` / `TriggerSpawnPoint`)。`spawn_y` は数値入力のみ。
- **削除**: `DeleteTriggerButton` が `remove(index)` して `history.record()`。

## base 画像 import

- **`CreateLevelButton`**: 新規作成と同時に `repo.import_base_image(name, source)` → `Level.base = basename`。Level 作成が失敗したら `repo.delete_base_image` でロールバックする (マルチステップの原子性、character の sprite import と同方針)。
- **`EditBaseInline`**: 既存 Level の差し替え。import → `Level.base` 更新 → `repo.save` → cache buster bump。

## エラーハンドリングと数値入力

- inline / modal とも `error: Signal<Option<String>>` を持ち、`alert alert-error` で表示する。parse 失敗・repo 呼び出し失敗・重複検出をここに集約する。
- 数値はすべて String Signal で保持し、Apply / Submit 時に `.trim().parse::<i32>()` (または `u32`) する。失敗時は `error.set(...)` で編集モードに留まる。

## 関連

- `entities/level/README.md`: Level 集約の型、`LevelRepository` の write API、永続化レイアウト
- `widgets/level/ui/README.md`: Canvas / Inspector / Detail (本 features を載せる widget 側)
- `features/character/ui/README.md`: UI パターン (A / B / C / D) の原典
- ADR-0008 (NavigationGuard) / ADR-0009 (キーボードショートカット) / ADR-0010 (snapshot History) / ADR-0015 (画像 cache busting)
