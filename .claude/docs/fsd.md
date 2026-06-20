# Feature-Sliced Design (FSD)

> Official reference: <https://feature-sliced.design/>

editor では FSD (Rust / Dioxus 版) を採用している。Rust の module system（可視性制御）が FSD の **encapsulation principle**（厳密な Public API 境界）をコンパイラレベルで強制してくれる。

---

## Layers

レイヤーは上から下へ順序付けられ、**上位レイヤーは下位レイヤーに依存できるが、下位レイヤーは上位レイヤーに依存してはならない**。

| # | Layer | 依存可能な layer | 役割 |
|---|-------|-----------|---------|
| 1 | **`app`** | すべての下位 | ブートストラップ、ルーティング、global provider、DI 配線 |
| 2 | **`pages`** | `widgets`, `features`, `entities`, `shared` | URL 単位のページ構成（thin controller） |
| 3 | **`widgets`** | `features`, `entities`, `shared` | 自己完結的な複合 UI ブロック（list/detail 描画） |
| 4 | **`features`** | `entities`, `shared` | ユーザー操作とユースケース（CRUD UI 等） |
| 5 | **`entities`** | `shared` のみ | ビジネスドメイン表現（struct + Repository trait） |
| 6 | **`shared`** | 外部 crate のみ | 再利用ユーティリティ、汎用ヘルパー |

> **Note**: `app` と `shared` には slice を持たず、segment 直下に置く。それ以外（`pages`, `widgets`, `features`, `entities`）は **slice → segment** の階層で分割する。

---

## Slices

slice はレイヤーを **業務ドメイン**（例: `character`）で分割する単位。各 slice は Rust の module ディレクトリ。

### Isolation Rule

**同じレイヤーの slice 同士は依存してはならない。**

```
// VIOLATION
features/create_character/  →  features/upload_asset/
entities/character/         →  entities/animation/

// OK
features/create_character/  →  entities/character/   (下位)
features/create_character/  →  shared/validation/    (下位)
```

### Cross-Slice Communication が必要な時

1. **Lift down**: 共通ロジックを `entities` か `shared` に降ろす（多くの場合「実は汎用だった」と気付く）
2. **Lift up**: 上位レイヤー（`widgets` か `pages`）が両 slice を協調させる

### 集約と slice の関係（重要）

DDD の **集約ルート** と slice 名を一致させる。例えば SpriteGroup / Animation / Frame / Layer / Role は Character の子集約 (構成要素) なので、`entities/sprite_group/` や `entities/animation/` を独立 slice にせず `entities/character/` 内（`model.rs` / `api.rs`）に同居させる。これにより同レイヤー間依存が発生しない。

**slice の中にさらに slice をネストするのも避ける**。たとえば `entities/character/animation/{model,api}.rs` のような構造は「Character slice の中に Animation slice を作っている」ことになり、FSD のセグメント規約から外れる。Animation / SpriteGroup の型と loader は Character の `model.rs` / `api.rs` に集約する（→ Anti-Patterns 5）。

---

## Segments

slice 内（または `app`/`shared` 直下）はコードを **技術的目的** でグルーピングする:

| Segment | 内容 |
|---------|---------|
| `ui` | UI コンポーネント、フォーマット、スタイル |
| `model` | データ構造、状態、ビジネスロジック |
| `api` | 外部システム（filesystem, network, webview）とのやり取り |
| `lib` | slice 内ヘルパー |
| `config` | 設定、feature flag |

必須ではなく、必要な segment だけ作る。1 ファイルで十分なら segment ファイルを 1 つだけ置く。

### engine の features は segment を作らない

editor の features layer は ui segment 配下に機能ファイル群を置く (`features/character/ui/create_character.rs` 等) が、engine の features は Bevy ECS の「Component + Resource + system + Plugin の束」を 1 ファイルにまとめる慣習で、`ui` に対応する自然な segment 名が無い。**engine の features では segment を作らず、slice 直下に purpose-based なファイル名を直接置く**。

```
features/character/
├── animation.rs     ← AnimationFrames + tick + AnimationPlugin
├── movement.rs      ← Player / WorldPosition / Facing + handle_input 等
└── state_machine.rs ← PlayerState + PlayerAnimationLibrary + sync_animation
```

`model` / `api` を features に持ち込まない:
- `model` は entities の中心 segment で永続化対象のデータ表現。features の Component / Resource は ECS runtime 専用の揮発状態で性格が違い、同名にすると「entities の model と features の model のどちらが本物のドメインか」が曖昧になる
- `api` は filesystem / network / webview など外部システムとの I/O。features の Bevy system は ECS 内のロジックで外部との yarit ではない (`handle_input` も `Res<ButtonInput>` を読むだけ)

### Segment Naming: Purpose over Essence

segment 名は「中身の言語構造」ではなく **「何を解決するか」** で命名する。

```
// GOOD — purpose-based names
shared/webview_assets.rs   // webview 経由でローカルアセットを読ませる仕組み
shared/config.rs           // 設定管理
shared/collision.rs        // 当たり判定の表現

// BAD — essence-based names (「何を入れる箱か」を述べているだけ)
shared/types.rs
shared/enums.rs
shared/hooks.rs
shared/components.rs
```

---

## Public API (Facade Pattern)

各 slice の facade ファイル（`{slice_name}.rs`）が Public API を定義する。Rust の `mod` + `pub use` で内部 segment を選択的に再エクスポートする。

```rust
// entities/character.rs — Facade
mod model;
pub use model::{Character, Sprite, SpriteGroup};

mod api;
pub use api::{CharacterRepository, FilesystemCharacterRepository, InMemoryCharacterRepository};
```

**Segments を `pub mod` として晒さない**。consumers は facade の `pub use` を経由してのみ slice / segment 内部にアクセスできる。これは `shared` も同じで、segment ファイルを `mod` で隠して必要な item だけ `pub use` する:

```rust
// shared.rs
mod webview_assets;
pub use webview_assets::{WORKSPACE_ASSET_SCHEME, WORKSPACE_ASSET_URL_PREFIX, workspace_asset_url};

mod collision;
pub use collision::{FlipMode, HitBox};

mod config;
pub use config::Config;
```

`pub mod segment;` で晒すと segment 内の `pub` item がすべて Public API になり、Rust の可視性で encapsulation を強制するという FSD の趣旨に反する。

### `app` レイヤーのパターン

`app` は最上位レイヤー。誰も import しない。すべての内部を `mod` で隠し、エントリポイントだけ公開:

```rust
// app.rs
mod app_root;
mod asset_handler;
mod characters_layout;
mod entrypoint;
mod routes;

pub use entrypoint::entrypoint;  // 唯一の Public API
```

### Import パス

consumer は常に slice の facade から import する:

```rust
// CORRECT — facade 経由
use crate::entities::character::Character;
use crate::features::character::CreateCharacterButton;

// WRONG — 内部 segment を直叩き
use crate::entities::character::model::Character;
```

---

## Rust 固有の規約

### ファイル命名: `slice.rs` + `slice/` ディレクトリ

Rust 2018+ のモジュール記法を採用。`mod.rs` は使わない:

```
entities.rs               ← entities レイヤーの facade
entities/
└── character.rs          ← character slice の facade
    └── character/
        ├── api.rs        ← segment ファイル
        └── model.rs
```

ファサードと中身ディレクトリが同じ階層に並ぶ形。

### 可視性

| Modifier | 用途 | 例 |
|----------|----------|---------|
| `pub` | Public API | `pub use model::Character;` |
| `pub(super)` | 同一 slice 内の sibling segment との共有 | `pub(super) struct InternalState` |
| `mod` (private) | デフォルト | `mod model;` |

---

## Layer ごとの公開パターン

| Layer | slice 持つ？ | facade のパターン | import 例 |
|:------|:------------|:-----------------|:-----------------------------|
| **`app`** | No | `mod` + `pub use` (entrypoint のみ) | `use crate::app::entrypoint;` |
| **`pages`** | Yes | `mod` + `pub use` (page component) | `use crate::pages::CharacterDetailPage;` |
| **`widgets`** | Yes | `mod` + `pub use` | `use crate::widgets::character::CharactersSidebar;` |
| **`features`** | Yes | `mod` + `pub use` | `use crate::features::character::CreateCharacterButton;` |
| **`entities`** | Yes | `mod` + `pub use` | `use crate::entities::character::Character;` |
| **`shared`** | No | `mod` + `pub use`（purpose-based segment 直） | `use crate::shared::workspace_asset_url;` |

---

## Anti-Patterns

### 1. Deep import（facade を迂回）

```rust
// WRONG
use crate::entities::character::model::Character;

// CORRECT
use crate::entities::character::Character;
```

### 2. segment を `pub mod` で晒す

```rust
// WRONG
// entities/character.rs
pub mod model;
pub mod api;

// CORRECT — Facade pattern
mod model;
mod api;
pub use model::Character;
pub use api::CharacterRepository;
```

### 3. Essence-based naming

```rust
// WRONG (中身の言語構造を述べているだけ)
mod types;
mod enums;
mod components;

// CORRECT (purpose: 何を解決するか)
mod webview_assets;
mod collision;
mod config;
```

### 4. 同一レイヤー間 import

```rust
// WRONG — features 間
use crate::features::other_feature::SomeThing;
// → "lift down" して entities/shared に降ろすか、上位 layer で協調させる

// WRONG — entities 間（DDD の別集約を slice 化した場合）
use crate::entities::other_aggregate::SomeThing;
// → 集約関係なら同じ slice に統合する
```

### 5. slice 内サブスライス

```
# WRONG — slice 内に slice をネスト
entities/character/
├── model.rs              (Character)
├── animation/
│   ├── model.rs          (Animation, Frame, Layer)
│   └── api.rs            (Animation::load_from_file)
└── sprite_group/
    ├── model.rs
    └── api.rs

# CORRECT — segment レベルに集約
entities/character/
├── model.rs              (Character / Animation / Frame / Layer /
│                          SpriteGroup / SpriteEntry / Role 全部)
└── api.rs                (Character / Animation / SpriteGroup の loader 全部)
```

segment 内が膨らんだ場合の細分化は OK（例: `api/{filesystem, in_memory, repository, tests}.rs` のように **segment の implementation detail** として `api/` ディレクトリで分ける）。**slice の中に slice を作る**のはアンチパターン。

---

## 現プロジェクトの構造（実例）

```
src/
├── app.rs                              ← layer facade
├── app/
│   ├── app_root.rs                     ← AppRoot, AppMain
│   ├── asset_handler.rs                ← use_workspace_asset_handler
│   ├── characters_layout.rs            ← CharactersLayout (master-detail shell)
│   ├── entrypoint.rs                   ← entrypoint()
│   └── routes.rs                       ← Routes enum (Routable derive)
│
├── pages.rs                            ← layer facade
├── pages/
│   └── characters.rs                   ← CharactersIndex / CharacterDetailPage / CharacterSpriteGroupPage
│
├── widgets.rs                          ← layer facade
├── widgets/
│   └── character.rs                    ← character slice facade (ui segment 経由で再 export)
│       └── character/
│           └── ui.rs                   ← ui segment facade
│               └── ui/
│                   ├── characters_sidebar.rs   ← CharactersSidebar
│                   ├── character_detail.rs     ← CharacterDetail
│                   ├── sprite_group_detail.rs  ← CharacterSpriteGroupDetail
│                   └── sprite_thumbnail.rs     ← SpriteThumbnail
│
├── features.rs                         ← layer facade
├── features/
│   └── character.rs                    ← character slice facade (ui segment 経由で再 export)
│       └── character/
│           └── ui.rs                   ← ui segment facade
│               └── ui/
│                   ├── create_character.rs     ← CreateCharacterButton + Modal
│                   ├── edit_character.rs       ← EditCharacterButton + Modal
│                   └── delete_character.rs     ← DeleteCharacterButton + ConfirmModal
│
├── entities.rs                         ← layer facade
├── entities/
│   └── character.rs                    ← character slice facade
│       └── character/
│           ├── api.rs                  ← CharacterRepository trait + InMemory + Filesystem
│           ├── model.rs                ← Character + SpriteGroup + Sprite struct
│           └── refresh.rs              ← CharactersRefreshTrigger + use_characters_refresh / use_characters_refresh_provider
│
└── shared.rs                           ← layer facade（slice なし、segment 直）
    └── shared/
        ├── collision.rs                ← HitBox, FlipMode
        ├── config.rs                   ← Config (workspace_dir)
        └── webview_assets.rs           ← workspace_asset_url + scheme/prefix 定数
```

依存方向の確認: すべて下向き（上位 → 下位）。同レイヤー間の slice 依存はゼロ。

---

## Quick Checklist (新コード追加時)

1. **正しい layer を選ぶ**: ドメイン (`entities`)、ユーザー操作 (`features`)、複合 UI (`widgets`)、ページ (`pages`)、shell (`app`)、汎用 (`shared`)
2. **slice を選ぶ or 作る**: ドメイン概念で分け、技術的類似性で分けない
3. **垂直依存をチェック**: 下位 layer の項目しか `use` しない
4. **水平隔離をチェック**: 同レイヤーの sibling slice は import しない
5. **Facade 経由で公開**: `mod` で隠し、`pub use` で必要なものだけ再エクスポート
6. **Deep import しない**: 常に slice の facade から
7. **Purpose で命名**: 特に `shared` の segment は「何を解決するか」で

---

## Go 写像（`packages/engine/`）

engine（→ ADR-0016）も同じ FSD を採用するが、Go の package モデル（1 ディレクトリ = 1 package）が Rust の facade パターンと異なるので、独自の写像規約で運用する（→ ADR-0018）。

### Layer / Slice / Segment の対応

| 概念 | Rust (editor) | Go (engine) |
|---|---|---|
| Layer | `src/{layer}.rs` + `src/{layer}/` | `internal/{layer}/`（ディレクトリのみ） |
| Slice | `src/{layer}/{slice}.rs` + `src/{layer}/{slice}/` | `internal/{layer}/{slice}/` = **1 Go package** |
| Segment | `src/{layer}/{slice}/{segment}.rs` または `{segment}/` | ファイル名 `{segment}.go`、複数化したらサブパッケージ |
| Facade | `{slice}.rs` で `mod` + `pub use` | **無し**（package privacy + `internal/` 階層で代替） |

### `entities/character/` の対応例

```
# editor (Rust)                         # engine (Go)
src/entities/character.rs               internal/entities/character/
src/entities/character/                 ├── model.go
├── model.rs                            ├── loader.go        (api segment 単純版)
├── refresh.rs                          ├── playback.go
├── playback.rs                         ├── boxes.go
└── api/                                └── README.md
    ├── repository.rs
    ├── filesystem.rs
    └── in_memory.rs
```

editor の `model.rs` ↔ engine の `model.go`、editor の `playback.rs` ↔ engine の `playback.go` のように **ファイル名を 1:1 対応** させる。これにより両側を同じ PR で更新する規律が機械的に決まる。

### Public API 境界の代替

facade を使わない代わりに:

1. **package privacy**: 小文字始まり = package private、大文字始まり = exported
2. **`internal/` 階層**: Go の `internal/` 規約により、engine module の外から `engine/internal/...` を import できない（誤った deep-import を Go コンパイラが拒否）
3. **同レイヤー間 import の禁止**: 言語強制ではない。初期は人力レビュー、将来 `revive` 等の lint で自動化（`packages/engine/.golangci.yml`）

### サブパッケージ化のタイミング

1 segment が 1 ファイルで収まる間は **flat に書く**。複数ファイルが必要になった時点で初めてサブパッケージ化する（editor の `entities/character/api/` と同じ）。

例:
- 現状: `internal/entities/character/loader.go`（1 ファイル、約 100 行）
- 将来: loader が `filesystem.go` / `in_memory.go` / `repository.go` に分かれる必要が出たら `internal/entities/character/loader/` package を切る

### Go 型と Rust 型の写像

| Rust | Go | 備考 |
|---|---|---|
| `u32` / `i32` / `f32` | `uint32` / `int32` / `float32` | サイズを揃えると YAML 互換性が壊れない |
| `String` | `string` | |
| `Vec<T>` | `[]T` | |
| `Option<T>` | `*T` | nil / non-nil で 2 状態 |
| `Option<Vec<T>>` | `*[]T` | **3 状態（nil / 空 / 値あり）を区別できる**（→ ADR-0014） |
| `[i32; 2]` | `[2]int32` | |
| `enum FlipMode { ... }` | `type FlipMode string` + 定数 | YAML 互換性のため文字列ベース |

YAML タグは editor の `serde` 属性と同じ snake_case 規約: `yaml:"sprite_group_number"` 等。

---

## References

- Official documentation: <https://feature-sliced.design/>
- Layers reference: <https://feature-sliced.design/docs/reference/layers>
- Slices & Segments: <https://feature-sliced.design/docs/reference/slices-segments>
- Public API: <https://feature-sliced.design/docs/reference/public-api>
- ADR-0001: Adopt Feature-Sliced Design (Rust 側)
- ADR-0018: FSD の Go 写像（engine 側）
