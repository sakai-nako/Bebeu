# entities/character — Character 集約

Character は editor 全体の **集約ルート**。FSD slice = DDD aggregate のルール（→ ADR-0003）に従い、Character の子エンティティ（SpriteGroup / Sprite / Animation / Frame / Layer）はすべてこの slice 内に同居する。同レイヤー間の依存は発生しない。

## ファイル構成

| ファイル | Segment | 役割 |
|---|---|---|
| `model.rs` | model | 集約のデータ構造（Character / SpriteGroup / Sprite / Animation / Frame / Layer + SelectedBox） |
| `api.rs` | api facade | `mod` 宣言と `pub use` の再エクスポートのみ |
| `api/repository.rs` | api | `CharacterRepository` trait + `ImportOutcome` |
| `api/in_memory.rs` | api | `InMemoryCharacterRepository` (テスト用 fake) |
| `api/filesystem.rs` | api | `FilesystemCharacterRepository` (本番用) |
| `api/tests.rs` | api | trait 契約テストと回帰テスト (両 impl 共通) |
| `refresh.rs` | api 補助 | `CharactersRefreshTrigger`（→ ADR-0004） |

`character.rs` の facade で必要な型だけを `pub use` する。

## 集約構造

```
Character (aggregate root)
├── name, thumbnail_path, hp, depth             ← metadata
│                                                  depth は world Z 厚みフォールバック (ADR-0024)
├── sprite_groups: Vec<SpriteGroup>             ← child (動的にロード、yml には書かれない)
│   └── SpriteGroup
│       ├── name, number
│       └── sprites: Vec<Sprite>
│           └── Sprite
│               ├── index, path, pivot_point
│               ├── body_boxes: Option<Vec<HitBox>>     (HitBox は depth: Option<u32> を持つ)
│               └── attack_boxes: Option<Vec<HitBox>>
└── animations: Vec<Animation>                  ← child (動的にロード)
    └── Animation
        ├── name, number, is_loop, loop_start_index
        └── frames: Vec<Frame>
            └── Frame
                ├── index, duration, flip
                ├── pivot_point_offset, body/attack_box_overrides
                └── layers: Vec<Layer>
                    └── Layer { sprite_group_number, sprite_index, transparency, ... }
```

`Character.depth` は world Z 軸の厚み (奥行き) のベース値。各 `HitBox.depth` が `None` の
ときフォールバックする (ADR-0024)。既定値 `DEFAULT_CHARACTER_DEPTH = 16` は `model.rs` で定義し、
`#[serde(default)]` により YAML 省略時に補完される。

### 参照は number / index ベース（name / filename ではない）

`Layer.sprite_group_number` + `Layer.sprite_index` で sprite を参照する。**SpriteGroup を rename しても、Sprite の filename を変えても Animation は壊れない**設計。

`Sprite.index` の重複は避ける（`SpriteGroup` 内で一意）。新規追加時は既存 max + 1 を採用。

## 永続化レイアウト

Single Source of Truth は **各子集約ファイル**。`{name}.yml` には `sprite_groups` / `animations` フィールドを書かない（`#[serde(skip)]`）。Repository::get で動的に walk して populate する。

```
workspace/data/characters/
├── MooR_01.yml                        ← Character metadata only (name, thumbnail_path, hp)
└── MooR_01/
    ├── sprite-groups/
    │   ├── walk.yml                   ← SpriteGroup metadata + sprites リスト
    │   └── walk/sprites/walk_001.png  ← 実バイナリ（asset_handler 経由で配信）
    └── animations/
        └── walk.yml                   ← Animation metadata + frames + layers
```

### 子集約ディレクトリの所有関係

`sprite-groups/` 配下には `*.yml`（metadata）と `{group}/sprites/*.png`（実バイナリ）が同居する。**書き込み責務は分かれる**:

| パス | 書き込みオペレーション |
|---|---|
| `sprite-groups/*.yml` | `create` / `update` |
| `sprite-groups/{group}/sprites/*` | `import_sprite_image` / `delete_sprite_image` |
| `animations/*.yml` | `create` / `update` |
| `animations/{anim}/...`（将来用） | 別オペレーション |

このため `update()` は子集約ディレクトリを `remove_dir_all` で丸ごと消してはいけない。`*.yml` のうち現在の集約名に含まれないものだけを削除し、サブディレクトリは触らない方針。

リグレッションテスト:

- `filesystem_update_preserves_sprite_image_files`
- `filesystem_update_preserves_files_under_animation_subdirs`

## Repository write API の責務範囲

5 種類の write メソッドがあり、触る対象が異なる:

| メソッド | `{name}.yml` | `sprite-groups/*.yml` | `sprite-groups/{group}/sprites/*` | 主な用途 |
|---|:-:|:-:|:-:|---|
| `create` | 新規作成 | 全件書き込み | 触らない | Character 新規作成 |
| `update` | 上書き | **差分同期**（不要 yml は削除） | 触らない | sprite_groups を含む batch 編集 |
| `update_metadata` | 上書き | **触らない** | 触らない | inline 編集（HP 等の単項目変更） |
| `rename` | move + name 書換 | （ディレクトリごと move） | （ディレクトリごと move） | Character の改名 |
| `delete` | 削除 | （ディレクトリごと削除） | （ディレクトリごと削除） | Character の削除 |
| `import_sprite_image` | 触らない | 触らない | コピー作成 | サムネイル等の画像取り込み |
| `delete_sprite_image` | 触らない | 触らない | 単一ファイル削除 | `import_sprite_image` のロールバック |
| `update_animation` | 触らない | 触らない | 触らない | AnimationEditor の単一 yml 上書き保存（`animations/{anim}.yml` のみ） |

**inline 編集で `update` を呼ばない**: sprite-groups/*.yml が全部書き直されて mtime が変わる。HP 等の単項目変更は `update_metadata` を使う。

**AnimationEditor の保存で `update` を呼ばない**: 編集対象の Animation 1 件だけを書きたいので `update_animation` を使う（他の animations/*.yml と sprite-groups の全 yml に余計な mtime 変動を起こさない）。

子集約専用の write メソッド（`rename_sprite_group` / `delete_sprite_group` / `rename_animation` / `delete_animation` / `update_animation`）も用意してあり、Character 全体を書き直さずに該当箇所だけ操作できる。

## list と get の非対称性

```rust
fn list(&self) -> Result<Vec<Character>>;        // sprite_groups / animations は空
fn get(&self, name: &str) -> Result<Option<Character>>;  // 子集約も完全にロード
```

サイドバー一覧で全 Character の SpriteGroup を読むのは無駄なので、`list` は metadata だけ返す。詳細ページは `get` で完全にロードする。

## Refresh トリガー

`CharactersRefreshTrigger` を AppMain で provide。features 層が mutation 後に `bump()` し、`CharactersLayout` の `use_effect` が再フェッチする。設計理由は ADR-0004。

## テストポリシー

- `*RepositoryContract`: trait のレベルで Repository が満たすべき不変条件をまとめたテスト関数群。InMemory と Filesystem の両方で同じテストを通す（`in_memory_repository_satisfies_contract` / `filesystem_repository_satisfies_contract`）。
- ファイル所有関係の境界は個別のリグレッションテスト（前述）でカバー。
- リネーム系は重複検出 / 未存在検出のエッジケースをすべてテスト。

## 関連 ADR / リファレンス

- ADR-0003: Aggregate root maps to FSD slice
- ADR-0004: Refresh trigger as wrapping counter
- ADR-0014: Frame override の 3-state encoding
- ADR-0024: world 3D AABB hit 判定 + per-box / character depth
- `.claude/docs/data-flow.md`: Repository / Signal / Refresh の全体像
