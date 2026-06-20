# OOUI × FSD Design Guidelines

## Overview

OOUI (Object Oriented User Interface) の原則を FSD アーキテクチャにマッピングするための指針。UI を作る時の primary reference。

**OOUI 原則**: ユーザーは先に **オブジェクト**（名詞）を選択し、その後 **アクション**（動詞）を実行する。task-oriented UI（動詞 → 名詞）の逆。

---

## OOUI → FSD レイヤーマッピング

| OOUI 概念 | FSD レイヤー | 説明 | 現プロジェクトの例 |
|-------------|-----------|-------------|---------|
| **Object（オブジェクト）** | `entities` | ドメインオブジェクトの表現 | `Character`、`SpriteGroup`、`Sprite` |
| **Action on Object** | `features` | オブジェクトに対するユーザー操作 | `CreateCharacterButton`、`DeleteCharacterButton` |
| **Collection View** | `pages` (+ `widgets`) | オブジェクト一覧/詳細のフルページ | `CharactersIndex`、`CharacterDetailPage` |
| **Composite Display** | `widgets` | 複数オブジェクトの複合 UI | `CharactersSidebar`、`CharacterDetail` |

### strict FSD との違い（重要）

[公式 Layers リファレンス](https://feature-sliced.design/docs/reference/layers) では **再利用性** を widgets / features の判別基準にしている:

> "If a block of UI...is never reused, it should not be a widget."
> "A good indicator that something can be a feature is the fact that it is reused on several pages."

本プロジェクトの editor は意図的にこの基準を外し、**OOUI 概念（Object / Action / Composite Display）を判別基準** にしている。CRUD ボタンは単一用途でも `features`、複合表示は単一用途でも `widgets` に置く。

これは strict FSD ではなく **OOUI × FSD の独自運用** なので、新しい UI を追加するときは「再利用されているか」ではなく「OOUI の何にあたるか」で配置を決める。

---

## Layout Structure

### Master-Detail パターン

エンティティページの基本レイアウト:

```
┌─────────────────────────────────────────────┐
│  CharactersLayout                           │
│ ┌────────────┬─────────────────────────┐    │
│ │ Sidebar    │ Main Area (Outlet)      │    │
│ │ (master)   │ (detail)                │    │
│ │            │                         │    │
│ │ + New      │ <selected の詳細表示>   │    │
│ │ ──────     │                         │    │
│ │ ○ Foo      │                         │    │
│ │ ● MooR_01 ←│                         │    │
│ │ ○ Bar      │                         │    │
│ └────────────┴─────────────────────────┘    │
└─────────────────────────────────────────────┘
```

- **Sidebar (master)**: そのカテゴリのオブジェクト一覧 + 新規作成ボタン
- **Main Area (detail)**: 選択中オブジェクトの properties / actions / 関連オブジェクト

実装は Dioxus router の `#[layout]` を活用し、サイドバー側を共通レイアウトコンポーネントに、ディテール側を `Outlet` で差し替える。

### 現状の Sidebar Organization

現在は Character のみ。`/characters` 配下が単一カテゴリ:

```
Characters
  + New
  ──────
  MooR_01     ← 選択中ハイライト（active_class）
  Foo
  Bar
```

将来 SoundGroup や Animation 等の独立エンティティが入った場合、上位の「カテゴリ切替サイドバー」を追加して 3 ペイン構造（カテゴリ / 一覧 / 詳細）に拡張する余地あり。

---

## Navigation Design

### Drill-down（親 → 子）

OOUI 的に「親オブジェクトを選択 → その子集約を選択 → 詳細表示」と階層を降りていく。URL 構造もそれに揃える:

```
/characters                                 ← 一覧（プレースホルダ）
/characters/:name                           ← Character 詳細（その中に sprite_groups リスト）
/characters/:name/sprite-groups/:group      ← SpriteGroup 詳細（sprite サムネ grid）
```

URL とファイルシステムが 1:1 対応:

```
URL                                                  ↔ Filesystem
/characters/MooR_01                                  ↔ workspace/data/characters/MooR_01.yml
/characters/MooR_01/sprite-groups/walk               ↔ workspace/data/characters/MooR_01/sprite-groups/walk.yml
```

### Breadcrumb

ネストが 2 階以上になった場合、現在地と帰路を提示するため breadcrumb を表示する。

```
MooR_01 > walk
   ↑ Link で /characters/MooR_01 に戻れる
```

実装: [widgets/character/sprite_group_detail.rs](../../packages/editor/src/widgets/character/sprite_group_detail.rs) の冒頭にある daisyUI `breadcrumbs` コンポーネント。

### サイドバー active 状態の prefix match

ネストルート（例: `/characters/MooR_01/sprite-groups/walk`）に居る時もサイドバーで `MooR_01` をハイライトしたい。Dioxus `Link` の `active_class` は **完全一致** のみなので、`use_route::<Routes>()` で現在ルートを取得して prefix match を自前で書く。

```rust
let active_name = match &current {
    Routes::CharacterDetailPage { name } | Routes::CharacterSpriteGroupPage { name, .. } => {
        Some(name.clone())
    }
    Routes::CharactersIndex {} => None,
};
```

active 判定ロジックは `app/` 層が持ち、widget には `active_name: Option<String>` だけ渡す（widget は `Routes` 型に依存しない）。

---

## Object Visual Representation

各オブジェクトの一覧/サムネ表現は対応する widget で実装:

| Object | List Item / Display | 実装場所 |
|--------|------------------|-------|
| **Character** | name のみ（背景色で active 表示） | [widgets/character/ui/characters_sidebar.rs](../../packages/editor/src/widgets/character/ui/characters_sidebar.rs) |
| **Character (Detail)** | name + thumbnail 画像 + HP (inline 編集) + sprite groups / animations の collapse カード | [widgets/character/ui/character_detail.rs](../../packages/editor/src/widgets/character/ui/character_detail.rs) |
| **SpriteGroup (in Character)** | `#番号` バッジ + name + sprite 数 | character_detail.rs 内のリスト |
| **SpriteGroup (Detail)** | name + number + sprite サムネ grid | [widgets/character/ui/sprite_group_detail.rs](../../packages/editor/src/widgets/character/ui/sprite_group_detail.rs) |
| **Sprite (in SpriteGroup)** | サムネ画像 + index + filename | [widgets/character/ui/sprite_thumbnail.rs](../../packages/editor/src/widgets/character/ui/sprite_thumbnail.rs) |
| **Animation (in Character)** | `#番号` バッジ + name + frame 数 | character_detail.rs 内のリスト |
| **Animation (Detail)** | name + number + loop 設定 + frame の grid | [widgets/character/ui/animation_detail.rs](../../packages/editor/src/widgets/character/ui/animation_detail.rs) |
| **Frame (in Animation)** | `#番号` + duration + 各 layer の sprite サムネ横並列 | [widgets/character/ui/frame_thumbnail.rs](../../packages/editor/src/widgets/character/ui/frame_thumbnail.rs) |

サムネ画像のローディングは [shared/webview_assets](../../packages/editor/src/shared/webview_assets.rs) の `workspace_asset_url()` でビルドした URL を `<img src>` に渡し、`use_asset_handler` 経由で webview に bytes を返す（[app/asset_handler.rs](../../packages/editor/src/app/asset_handler.rs)）。

---

## Action Placement

### Object-level Action（CRUD）

| Action | 配置場所 | UI パターン |
|--------|-------------|------------|
| Create | サイドバーの heading 横「+ New」ボタン | Modal（フォーム + 画像ピッカー） |
| Rename | 詳細ページの h1 横ボタン | Modal（new name 入力） |
| Delete | 詳細ページの h1 横ボタン | 確認 Modal |
| Inline 編集（HP 等） | 詳細ページの Properties 行内、各フィールドの隣に ✏ ボタン | クリックで input/Save/Cancel に切替 |
| Thumbnail 差替 | 詳細ページのサムネイル画像下のボタン | Modal（SpriteGroup + Sprite の 2 段選択） |

実装は features 層:
- [features/character/ui/create_character.rs](../../packages/editor/src/features/character/ui/create_character.rs)
- [features/character/ui/rename_character.rs](../../packages/editor/src/features/character/ui/rename_character.rs)
- [features/character/ui/delete_character.rs](../../packages/editor/src/features/character/ui/delete_character.rs)
- [features/character/ui/edit_hp_inline.rs](../../packages/editor/src/features/character/ui/edit_hp_inline.rs)
- [features/character/ui/change_thumbnail.rs](../../packages/editor/src/features/character/ui/change_thumbnail.rs)

### Modal の所有パターン

「親が開閉状態を持つ、modal は常に open として render される」パターンを採用:

```rust
// 親側
let mut show_modal = use_signal(|| false);
if show_modal() {
    CreateCharacterModal { onclose: move |()| show_modal.set(false) }
}
```

modal は visibility を意識しない。閉じる手段（`onclose: EventHandler<()>`）だけを受ける。閉じれば form state も破棄される。

### mutation 後の再描画

features 内の操作で Repository に書き込んだら `CharactersRefreshTrigger.bump()` を呼ぶ。詳細は [data-flow.md](data-flow.md)。

### Inline Action（子集約の編集）

現状未実装。SpriteGroup の中の Sprite を編集する場合（pivot 編集 / HitBox 編集等）、これらは独立した features ではなく **親集約 (Character) を `update()` する操作** として扱う。子集約だけの Repository は持たない。

---

## 集約関係と FSD slice の対応

DDD の集約構造に slice を揃える。同レイヤーの slice 間依存を発生させないため:

| 集約構造 | slice 配置 |
|---|---|
| Character (集約ルート) | `entities/character/` |
| SpriteGroup (Character の子集約) | 同 slice 内 (`entities/character/model.rs` に同居) |
| Sprite (SpriteGroup の子) | 同上 |
| HitBox (Sprite の値オブジェクト) | `shared/collision/`（複数集約から参照される汎用 value object） |

将来 Object や Level が独自の sprite groups を持つ場合、それらは Object/Level 集約の中に独自の SpriteGroup struct を持つ（DDD 的に「文脈の異なる別概念」とみなす）か、純粋な値型として `shared/` に降ろすかを選ぶ。

---

## 将来の拡張案（aspirational）

実装はまだだが、設計指針として残しておく。

### Specialized Editor View

詳細ページとは別の「専用エディタ」として、より広い作業空間を持つビューを開く。OOUI の二段階ナビゲーション:

```
1. /characters/:name/sprite-groups/:group     ← 通常の Master-Detail
2. /characters/:name/sprite-groups/:group/edit ← フルエディタ（canvas + 各種パネル）
```

実装の参考 (`/edit` ルート):

| 領域 | レイヤー | 役割 |
|---|---|---|
| EditorPage | `pages/` | レイアウト構成、ルートパラメータ受け取り |
| SpriteCanvas | `widgets/character/` | sprite 描画、HitBox/pivot のオーバーレイ |
| PropertyPanel | `widgets/character/` | 選択中要素のプロパティ編集 |
| Editing Actions | `features/character/` | pivot ドラッグ、HitBox 追加/削除等 |

### CharactersLayout を超えるエンティティ

別エンティティ（例: グローバルな設定、ライブラリ的アセット）を追加する場合は、`/characters` とは独立した layout を作る。サイドバーの上位にカテゴリ切替を入れる構造に拡張可能。
