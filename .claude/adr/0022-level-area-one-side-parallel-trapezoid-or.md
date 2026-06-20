# ADR-0022: Level Area as one-side-parallel trapezoid list with OR composition

## Status

Refined by [ADR-0023](0023-image-pixel-world-screen-unification.md)

> 台形 + OR 合成の判断は有効。ADR-0023 で座標系が「画像ピクセル = world」に一本化
> され、Z の向きが反転した（near=手前=画像下=大、far=奥=画像上=小）ため、不変条件が
> `near_z <= far_z` → `near_z >= far_z` に、判定式の範囲ゲートが反転している。

## Context

ADR-0017 で world は 3 軸 (X / Y=高さ / Z=奥行き) で持ち、`screen_y = level.GroundScreenY - world_y - world_z * level.ZScale` で 2.5D 投影することを決めた。Beat 'em up の足場（プレイヤーが移動できる XZ 平面上の領域）を Level YAML でどう表現するかが、その次に必要な決定だった。

要件:

- 2.5D 俯瞰で「奥が狭く / 手前が広い」あるいは逆の遠近感を表現したい（直線パースの近似）
- editor 側は Canvas で base 画像の上に Area を描画 / 編集できる必要がある。テキスト編集（YAML 直書き）でも扱える単純さがほしい
- engine 側の hit / movement で `Contains(x, z)` を高頻度に呼ぶので、判定コストは低いほど良い
- 通路の合流・分岐・複数の島のような複雑領域も最低限表現できる

## Decision

移動可能領域を **1 辺平行台形のリスト + OR 合成** で表現する。

`Area` を以下のように定義する:

- 上下 2 辺 (Z = `near_z` / `far_z`) は **必ずスクリーン水平に平行**（= 同じ Z 値の直線セグメント）
- 左右 2 辺だけが斜めにできる（一般の台形ではなく、上下が平行な「1 辺平行台形」に限る）
- `near_min_x == far_min_x && near_max_x == far_max_x` のとき矩形に縮退する（特殊扱いしない）
- 複数 Area は **OR 合成**（どれか 1 つに入っていれば移動可）
- engine 側で空リスト時の挙動は **制限なし扱い**（fail-soft。loader が Default を返さなかった場合に落ちないため）

判定式（`level.Area.Contains` / `level.ContainsAny`、Z は near>=far、ADR-0023）:

```
Z 上の補間係数 t = (z - near_z) / (far_z - near_z)
leftX  = near_min_x + t * (far_min_x - near_min_x)
rightX = near_max_x + t * (far_max_x - near_max_x)
inside = (far_z <= z <= near_z) && (leftX <= x <= rightX)
```

YAML 表現（実 Level の抜粋。Z = 画像ピクセル Y で near=手前=画像下が大）:

```yaml
areas:
  - near_z: 207.24168
    far_z: 153.60833
    near_min_x: 0.0
    near_max_x: 905.5333
    far_min_x: 0.0
    far_max_x: 905.86664
```

## Alternatives Considered

- **矩形 (AABB) のみ (`near_min_x == far_min_x && near_max_x == far_max_x` 固定)**
  - 実装はもっとも単純。`Contains` も `x ∈ [min_x, max_x] && z ∈ [near_z, far_z]` で済む
  - **奥に行くほど狭くなる / 手前が広がる遠近感**が出せない。Beat 'em up の俯瞰演出では台形が標準であり、ステージごとのアートディレクションを縛ってしまう

- **一般のポリゴン（凸 / 凹）**
  - 任意の形を表現できる
  - editor 側で頂点をドラッグするマップエディタが必要になり、YAML テキスト編集との両立が崩れる
  - `Contains` 判定が point-in-polygon になり、辺数 × 領域数のコストがかかる
  - 通路の枝分かれを 1 ポリゴンで凹形にして表現するのは可能だが、YAML での読み書きが非直感的

- **単一 Area（リストにしない）**
  - もっとも単純なスキーマ。多くのステージで 1 領域で足りるのも事実
  - 「通路の合流」「分かれた島」「base 画像にめり込む障害物を避ける」ような複合領域が表現できない。Level ごとに「単一 → 複数」のスキーマ変更が後から必要になる可能性が高く、最初から `Vec<Area>` にしておくのが将来コストが低い

- **スプライン / Bezier 境界**
  - 滑らかな曲線で囲める
  - editor 側に専用エディタが必要、YAML 表現も冗長。Beat 'em up の足場は基本的に直線で十分

- **base 画像のアルファマスクを領域として使う**
  - データを画像 1 枚に統合できる
  - 編集時の解像度依存（pixel 単位）、エンジン側で画像を読まずに判定したいケースに使えない、scale 違いの Level で再利用しづらい

## Consequences

**得られたもの**

- `Contains` 判定が **線形補間 1 回 + 比較 4 回**で済む。entity 数 × Area 数のループでも軽量
- editor の Canvas 編集 UI に落とすときの自由度が下がる代わりに、操作対象が「4 つの X 値 (`near_min_x`, `near_max_x`, `far_min_x`, `far_max_x`) + 2 つの Z 値」に固定され、ハンドル数が少なく直感的
- YAML テキスト編集だけでも完結する（特に近似的なステージ初期作成）
- 矩形と台形を **同じ型で表現** できる（縮退ケースを特殊化しない）。Default Area は矩形相当
- `Vec<Area>` を OR 合成することで「通路 + 別の足場」のような複合領域も表現可能。空リストは制限なし扱いとし、Default Level が来なかった場合のフェイルセーフにする

**支払うコスト / 注意点**

- カーブした足場（円形のフィールド境界など）は 1 つの Area で近似できず、台形を複数並べる必要がある
- 上下辺が水平に制約されるので、傾いた「斜めの足場」は表現できない（カメラ視点を傾けない俯瞰想定なので問題視しない）
- Area がオーバーラップしても hit 判定では問題ないが、editor 側の Canvas 描画では重なりの視認性に注意が必要（半透明レンダリング）
- `ContainsAny` の空リスト = 制限なし扱いは debug 時の罠になりやすい（移動制限が利かないように見える）。loader のテストで Default Areas が必ず入ることをカバーする

**関連 ADR**

- ADR-0023: base 画像ピクセル = world(X,Z) の一本化（`near_z >= far_z` への反転、Z = 画像ピクセル Y）
- ADR-0017: world 軸 (X / Y=高さ / Z=奥行き) と 2.5D 投影（`near_z` / `far_z` の意味付け、ADR-0023 で supersede）
- ADR-0011: filesystem YAML を一次ストレージとする（Area の表現は YAML 直書きで扱えること）
- ADR-0003: 集約ルート = slice（Area は Level の値型として同 slice 内）
