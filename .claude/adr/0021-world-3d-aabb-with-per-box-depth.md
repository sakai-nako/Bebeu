# ADR-0021: hit 判定を world 3D AABB + per-box / character depth に移行する

## Status

Accepted (ADR-0018 を refine)

## Context

ADR-0018 で hit 判定を screen 空間 AABB にした理由は「editor の box 編集 UI が sprite-pixel 平面のみで、Z 厚みを表現する手段が無いため」だった。Stage 2 で world Z 軸を導入してからは、Z 違いの「殴りにくさ」は ADR-0017 の Y 投影 (`screen_y = ground - world_y - world_z * z_scale`) で暗黙的に得られていた。

Stage 4 が見え始めたタイミングで以下の懸念が浮上した:

1. **camera 導入で screen 空間判定が壊れる**: Stage 3+ で `screen_x = world_x - cam.X` に拡張する想定がある。「ズームやスクロールで hit 結果が変わる」のは仕様バグであり、screen AABB のままでは camera を入れた瞬間に挙動が破綻する
2. **broad-phase との接続**: entity が増えた時 quadtree / BVH で O(N²) を潰す段階で、世界座標の AABB がそのまま空間インデックスに乗るのに対し、screen AABB はフレーム毎に投影してから index する二重コストが発生する
3. **奥行き許容度の制御性**: 「Y 投影による暗黙の縮退」は Level の `ZScale` 1 つに依存するため、box ごとの「奥行きの当たりやすさ差」(細い剣 vs 広い踏みつけ等) を表現できない
4. **camera 非依存 / determinism / 同期**: world 座標は camera, resolution, replay の差異に強く、将来のネットワーク同期や replay 再現性の観点でも素直

editor 側の box 編集 UI に Z 軸操作を新設するコストは大きい (3D canvas が要る) ので、box 編集は **XY 平面の 2D エディタのまま**、Z は **数値 1 つの厚みフィールド** として表現することで両立を図る。

## Decision

**world 座標系での 3D AABB 重なりで hit 判定する。Z 厚みは Character ベース値 + 各 HitBox の Option 上書きで表現する。**

### データモデル

```rust
// editor: shared/collision.rs
pub struct HitBox {
    top_left: [i32; 2],
    bottom_right: [i32; 2],
    /// world Z 厚み。None なら所属 Character.depth にフォールバック
    #[serde(default, skip_serializing_if = "Option::is_none")]
    depth: Option<u32>,
}

// editor: entities/character/model.rs
pub struct Character {
    // ...
    /// HitBox.depth が None の box が参照するベース値 (既定 16)
    #[serde(default = "default_character_depth")]
    pub depth: u32,
}
```

```go
// engine: shared/hitbox/hitbox.go
type HitBox struct {
    TopLeft     [2]int32
    BottomRight [2]int32
    Depth       *uint32 // nil なら Character.Depth にフォールバック
}

// engine: entities/character/model.go
type Character struct {
    // ...
    Depth uint32 // 既定 16 (loader が 0 を DefaultDepth に補正)
}
```

### Z 範囲のセマンティクス

box の Z 範囲は **character.PosZ を中心に対称**に展開する:

```
[char.PosZ - depth/2, char.PosZ + depth/2]
```

`depth` は HitBox に書かれていればそれ、`None` (Rust) / `nil` (Go) なら所属 Character の `depth` を使う (`HitBox::resolved_depth(character.depth)` ヘルパーで集約)。

### 座標変換

editor の box は引き続き sprite-pixel 単位 (XY、原点は character の足元 = sprite pivot)。engine 側の `ResolveWorldBoxes` で world AABB に変換する:

```
world_x = char.PosX + sprite_pixel_x   (FlipH 適用後)
world_y = char.PosY - sprite_pixel_y   (sprite Y は下向き、world Y は上向き)
world_z range = [char.PosZ - depth/2, char.PosZ + depth/2]
```

flip は X 軸のみ (`2 * char.PosX - x` で反転)。Y / Z は影響しない。

### hit 判定

`features/attack.Hits` は `ResolveWorldBoxes` の結果を `hitbox.AnyOverlap3D` にかける。3 軸すべてで strict overlap (端一致は重なり扱いしない) なら hit。

screen AABB 用の `ResolveScreenBoxes` は **debug overlay 描画専用** に残す (削除しない)。overlay は引き続き screen 空間で描画する必要があるため、両解決を併設して責務を分ける。

### 既定値 / 0 取り扱い

- editor: `DEFAULT_CHARACTER_DEPTH = 16`、Character.depth は serde の `#[serde(default)]` で YAML 省略時に補完
- engine: `DefaultDepth = 16`、loader が `c.Depth == 0` を `DefaultDepth` に補正
- HitBox.depth は **0 を許容** (= 厚みゼロで原理的に当たらない、特殊ケース)。「YAML に depth フィールドが無い」と「depth: 0」を区別するため Option (Rust) / pointer (Go) で表現する

### editor UI

depth 入力は 3 ヶ所に追加:
- **CharacterDetail の Properties** に Depth (Z) 行 + `EditDepthInline` (HP の inline 編集と同 Pattern)
- **SpritePropertyPanel の BoxEditor** の 4 角入力の下に Depth (Z) 入力。空欄でフォールバック (placeholder に `{character.depth} (inherit)` を表示)
- **AnimationPropertyPanel の Frame override BoxRow** の最後尾に "Z" ラベル + Depth 入力

`HitBoxDepthInput` 共通 component で 3 ヶ所の挙動 (空文字 → None、整数 → Some(n)、同値ならコールバック呼ばない) を統一する。

## Alternatives Considered

- **screen AABB + Z gate (`|att.Z - def.Z| > thresh` で早期 false)**:
    - 既存実装に最も近く、Stage 3 のうちは小回りが利く
    - 二重 culling (Y 投影による暗黙縮退 + 明示 Z gate) で tuning が複雑
    - camera 導入時に screen AABB 部分が破綻する点は変わらず、結局 Stage 4 で再リファクタが必要
    - 「per-box の Z 制御」は Z gate の閾値だけでは表現しきれない

- **完全な 3D AABB だが Z 厚みを Character.depth のみ (per-box override 無し)**:
    - データモデルが最小化
    - 「細い剣 vs 広い掃き払い」のような box 単位の Z 表現力差を出せない。Stage 4 のアクション設計の自由度が下がる
    - 将来 per-box にしたくなったら HitBox 構造体 (= YAML スキーマ) を変える破壊的変更になる

- **HitBox に直接 (z_min, z_max) を書く** (世界 Z 範囲を絶対値で):
    - 「キャラの足元から見て前 / 後ろ どれくらい」を意識せずに済む
    - editor 側で「box ごとに絶対 Z 値」を入力する操作が直感的でない (XY が pivot 相対なのに Z だけ絶対値、という非対称)
    - depth (= 厚み) で持ち、対称展開は engine 側に閉じる現案のほうが対称的

- **非対称 (`depth_front`, `depth_back`)**:
    - 「斜め前への突き」のような表現が可能
    - YAML フィールドが倍、UI 入力数も倍。Stage 4 開始時点では over-engineered
    - 将来必要になれば `Option<u32>` を `Option<DepthRange>` に拡張する形で増やせる (現案の互換性は維持しやすい)

- **編集 UI に 3D canvas を導入**:
    - 表現力は最大
    - 開発コストが大きく、Stage 4 のスコープ外。本 ADR の目的は「Stage 4 開始までに hit 判定を world 化する」こと

## Consequences

**得られたもの**

- camera / resolution / Level 設定 (ZScale) の変化に対して hit 結果が不変 (= 仕様の安定性)
- broad-phase (quadtree / BVH) との接続が自然: world AABB をそのまま空間 index に積める
- per-box / per-character の Z 厚みで「殴りやすい / 殴りにくい」を designer が制御できる
- ResolveScreenBoxes (overlay 描画用) と ResolveWorldBoxes (hit 判定用) の責務が分離し、それぞれ単体でテスト・拡張できる
- editor の YAML 互換性: depth 省略の既存 yml はそのまま読め、character.depth = 16 + box.depth = None で旧来挙動と意味的に近い動きになる (= ADR-0017 の Y 投影は描画にしか効かなくなったが、近い距離なら従来同様にヒットする)

**支払うコスト / 注意点**

- editor / engine 双方の HitBox / Character スキーマ変更 (新フィールド追加)。後方互換は serde default / loader 補正で確保しているが、書き戻し時 (yml save) には depth フィールドが追記される
- 既存 YAML で attack box が「ぎりぎり離れた Z で当たっていた」シーンは挙動が変わる可能性: ADR-0017 の Y 投影による暗黙の縮退から、明示的な Z 厚み判定に変わるため。MooR_01 程度の規模では再 tuning は最小限で済むはず
- box overlay 描画 (debug) と hit 判定で別関数を呼ぶ二重コードパスになる (`ResolveScreenBoxes` / `ResolveWorldBoxes`)。XY の式は両者で一致させる規約を test で守る必要がある
- `scaled` (再 import 倍率) は depth に **作用しない**。再 import で character のサイズ感が変わったら designer が手動で `Character.depth` を調整する必要がある
- editor UI: Z 厚みは画面に映らないので、Z gate に弾かれる box の存在に気付きにくい。当面は overlay (XY のみ) と「Depth (Z)」の数値表示だけで運用するが、将来 mini-map / top-down preview の追加を検討してもよい

**今後の拡張余地**

- 非対称 Z (`Option<DepthRange { front, back }>` 等) に拡張する場合、HitBox.depth を sum type に変えるだけで済む
- broad-phase: world AABB を loose octree や per-axis SAP に乗せる場合、`AnyOverlap3D` を BVH ノード経由で呼ぶ形に差し替える
- per-frame / per-action 単位の depth multiplier (例: 「ジャンプ攻撃の attack box は base depth × 1.5」) を入れる場合、`ResolveWorldBoxes` の `makeWorldBox` で character.depth をスケールする層を 1 段足す
- 円柱判定 (Z 軸のみ円形) など非 AABB に拡張する場合は `WorldBox` の代わりに `WorldShape` (interface) を導入し、`Overlap3D` を多相化する
