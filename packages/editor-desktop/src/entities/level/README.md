# entities/level — Level 集約

Level は editor の **集約ルート**。FSD slice = DDD aggregate のルール（→ ADR-0003）に従い、Beat 'em up の 1 ステージ（旧 Stage）を 1 つの YAML にまとめる。`workspace/data/levels/` の master pool として全 Project から共有される。

## ファイル構成

| ファイル | Segment | 役割 |
|---|---|---|
| `model.rs` | model | 集約のデータ構造（`Level` / `Area` / `OpponentTrigger` + `with_defaults`） |
| `api.rs` | api | `LevelRepository` trait + `InMemoryLevelRepository` / `FilesystemLevelRepository` + contract テスト |
| `provider.rs` | api 補助 | Dioxus context で `HashMap<String, Level>` を配布 |
| `refresh.rs` | api 補助 | `LevelsRefreshTrigger`（→ ADR-0004） |

facade は [level.rs](../level.rs) で `mod` + `pub use` する。

## 集約構造

```
Level (aggregate root)
├── name, base, base_dimensions          ← metadata (world(X,Z) = base 画像ピクセル, ADR-0026)
├── camera_start_x / camera_start_y      ← Level 開始時のカメラ視界左上 (画像ピクセル)
├── player_spawn_x / player_spawn_z      ← Player 初期 spawn (Y は常に 0)
├── player_respawn_y                     ← 死亡後の再 spawn 落下開始 Y
├── areas: Vec<Area>                     ← 移動可能領域 (OR 合成、→ ADR-0025)
└── opponent_triggers: Vec<OpponentTrigger>
```

`Area` / `OpponentTrigger` は値型。Character のような子集約 yml / ディレクトリは持たず、すべて単一 yml に同居する。

### Area

XZ 平面上の **1 辺平行台形**。`near_z` / `far_z`（手前 / 奥 Z = 画像ピクセル Y）の 2 辺はスクリーン水平に平行で、左右の辺だけ斜めにできる。near=手前=画像下が大きく `near_z >= far_z`。`near_min_x == far_min_x && near_max_x == far_max_x` のとき矩形に縮退する。複数 Area は OR 合成（どれか 1 つに入っていれば移動可）。代替案と意思決定の経緯は ADR-0025 / ADR-0026。

### OpponentTrigger

Player の world X が `trigger_x` 以上になった瞬間に 1-shot 発火し、`(spawn_x, spawn_y, spawn_z)` に `character_name` の Character を 1 体生成する。`character_name` は **文字列参照**で、Character pool 側の rename / delete は editor 上の警告表示のみで自動追従しない（master pool の独立操作。Project の `levels[]` 参照と同じ規約）。

## `base_dimensions` の非対称扱い

`Level.base_dimensions: Option<[u32; 2]>` は YAML に書かない（`#[serde(skip)]`）。`FilesystemLevelRepository` が `data/levels/{name}/{base}` の PNG header から読み込んで注入する。PNG 以外 / 不在 / 破損は `None` のまま動作する（area / spawn の画像内 clamp が外れるだけで落ちない）。YAML と画像で同じ情報を二重管理しない。

## 永続化レイアウト

```
workspace/data/levels/
├── {name}.yml                    ← Level 本体 (name はファイル名由来、YAML には書かない)
└── {name}/
    └── base.png                  ← Level.base が指す画像 (既定名 "base.png")
```

`{name}.yml` の例（`workspace/data/levels/ct.yml`）:

```yaml
base: base.png
areas:
  - near_z: 207.24168
    far_z: 153.60833
    near_min_x: 0.0
    near_max_x: 905.5333
    far_min_x: 0.0
    far_max_x: 905.86664
camera_start_x: 0.0
camera_start_y: 0.0
player_spawn_x: 53.800003
player_spawn_z: 183.575
player_respawn_y: 0.0
opponent_triggers:
  - character_name: MooR_01
    trigger_x: 0.0
    spawn_x: 245.4
    spawn_y: 0.0
    spawn_z: 175.975
```

- 空の `opponent_triggers` は YAML から省略される（`#[serde(skip_serializing_if = "Vec::is_empty")]`）。
- 未指定フィールドは `Level::with_defaults(name)` の値にフォールバックする（fail-soft、→ [.claude/docs/testing.md](../../../../../.claude/docs/testing.md)）。

## Repository write API

`LevelRepository` trait（[api.rs](api.rs)）は **load** と **get** を非対称に分けているのが Character との大きな違い:

| メソッド | 不在 / parse 失敗時 | 主な用途 |
|---|---|---|
| `load(name)` | `Level::with_defaults(name)` を返す（fail-soft） | engine 起動 / Project 詳細の summary / 未保存 Level でも開ける |
| `get(name)` | `Ok(None)`（parse 失敗時は default + name で `Some`） | 詳細編集ページ。「本当にファイルが存在するか」を区別したい場合 |
| `list()` | — | サイドバー一覧（sorted） |
| `create(level)` | 同名で error | Level 新規作成 |
| `save(level)` | — | 上書き保存（親ディレクトリは自動作成） |
| `rename(old, new)` | 旧不在 / 新衝突で error | Level の改名 |
| `delete(name)` | 不在で error | Level の削除 |
| `import_base_image(level_name, source)` | — | 外部画像を `{level_name}/base.{ext}` にコピー。拡張子は小文字化、同名は上書き |
| `delete_base_image(level_name, basename)` | 不在で no-op | `import_base_image` のロールバック用 |
| `exists(name)` | — | Create / Rename の事前重複チェック |

実装は本番用の `FilesystemLevelRepository`（`{workspace_dir}/data/levels/`）とテスト用 fake の `InMemoryLevelRepository`。後者は `import_base_image` が basename だけ返す簡易実装。

### Repository が触らないもの

- **Project YAML 側の `levels[]` 配列**: `rename` / `delete` で更新しない（master pool の独立操作）。Project 参照の追従は将来 warning UI で扱う。
- **`workspace/assets/`**: ユーザーの自由領域。`import_base_image` の source として読み取りはするが書き込みはしない（Character の sprite と整合）。

## 関連 features / widgets / pages

| Layer | 場所 | 役割 |
|---|---|---|
| features | [features/level/ui/create_level.rs](../../features/level/ui/create_level.rs) | 新規作成モーダル（名前 + base 画像 import） |
| features | [features/level/ui/rename_level.rs](../../features/level/ui/rename_level.rs) / [delete_level.rs](../../features/level/ui/delete_level.rs) | 改名 / 削除アクション |
| features | [features/level/ui/edit_base_inline.rs](../../features/level/ui/edit_base_inline.rs) | base 画像の差し替え inline 編集 |
| features | [features/level/ui/edit_camera_start.rs](../../features/level/ui/edit_camera_start.rs) / [edit_player_spawn.rs](../../features/level/ui/edit_player_spawn.rs) / [edit_player_respawn_y.rs](../../features/level/ui/edit_player_respawn_y.rs) | 座標フィールドの inline 編集 |
| features | [features/level/ui/opponent_triggers/](../../features/level/ui/opponent_triggers) | OpponentTrigger 行の section / row / delete |
| features | [features/level/ui/edit_level.rs](../../features/level/ui/edit_level.rs) | エディタ全体のアクション（Undo / Redo の record 単位） |
| widgets | [widgets/level/ui/level_detail.rs](../../widgets/level/ui/level_detail.rs) | Draft / History パターンの全体編集画面（Save 前は disk 非タッチ） |
| widgets | [widgets/level/ui/level_canvas.rs](../../widgets/level/ui/level_canvas.rs) | base 画像 + Area / 各点の Canvas 視覚化 |
| widgets | [widgets/level/ui/level_inspector.rs](../../widgets/level/ui/level_inspector.rs) | プロパティパネル |
| widgets | [widgets/level/ui/levels_sidebar.rs](../../widgets/level/ui/levels_sidebar.rs) | Level 一覧サイドバー |
| pages | [pages/levels.rs](../../pages/levels.rs) | URL → Level 解決の thin controller |

## Refresh トリガー

`LevelsRefreshTrigger` を AppMain で provide。features 層が mutation 後に `bump()` し、`LevelsLayout` の `use_effect` が再フェッチする。設計理由は ADR-0004。

## 参考ファイル

- [model.rs](model.rs): `Level` / `Area` / `OpponentTrigger` の型定義 + `with_defaults`
- [api.rs](api.rs): `LevelRepository` trait と FS / InMemory 実装、contract テスト
- [provider.rs](provider.rs): `LevelsContext` の Signal 配布
- [refresh.rs](refresh.rs): `LevelsRefreshTrigger`

## 関連 ADR

- ADR-0003: 集約ルート = slice
- ADR-0004: Refresh trigger as wrapping counter
- ADR-0011: filesystem YAML を一次ストレージとする
- ADR-0019: world 軸（X / Y=高さ / Z=奥行き）と 2.5D 投影（ADR-0026 で supersede）
- ADR-0025: Level Area as one-side-parallel trapezoid list with OR composition（ADR-0026 で near>=far に反転）
- ADR-0026: base 画像ピクセル = world(X,Z) = screen の一本化（投影パラメータ廃止）
