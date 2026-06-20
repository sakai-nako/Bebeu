# ADR-0023: base 画像ピクセル = world (X, Z) = screen 座標の一本化

## Status

Accepted（Supersedes [ADR-0017](0017-world-axes-and-25d-projection.md)）

## Context

ADR-0017 は world を 3 軸 (X / Y=高さ / Z=奥行き) で持ち、Level ごとの投影パラメータ `ground_screen_y` / `z_scale` を使って `screen_y = ground_screen_y - world_y - world_z * z_scale` で 2.5D 投影することを決めた。運用してみて、次の問題が表面化した。

- **座標系が 3 段（画像ピクセル → world → screen）あり、変換係数が Level ごとの `ground_screen_y` / `z_scale`**。「base 画像のどの座標が画面のどこに来るか」が直感的に追えない。
- **editor と engine で Z の符号が食い違っていた**。editor の Canvas は `image_y = ground_screen_y + world_z * z_scale`（Z+ が画像下）で描画していたのに、engine / ADR-0017 は `screen_y = ground_screen_y - world_z * z_scale`（Z+ が画面上）。world Y は両者とも反転していたが Z だけ非対称で、editor で奥に置いた Area / spawn が engine で手前に出る（奥行きが上下反転）バグになっていた。
- **背景描画が画像下端基準 `Translate(-camX, scrH-bgH-camY)`** で投影式（画像 top 基準）と食い違い、base 画像高さ ≠ 解像度高さのとき縦に `bgH - scrH` px ずれていた（ct: 224 vs 216 で 8px）。
- 全 Level で `z_scale = 1.0` 運用であり、`ground_screen_y` も実質「地面ラインの画像 Y」を画面 Y として再解釈しているだけだった。投影パラメータが抽象的な割に使われていない。

## Decision

**投影パラメータ `ground_screen_y` / `z_scale` を廃止し、base 画像ピクセル座標を world (X, Z) および screen 座標（camera offset 適用前）と一本化する。**

```
world_x = base 画像ピクセル x
world_z = base 画像ピクセル y      （Z+ = 画像下 = 手前 / Z 小 = 画像上 = 奥）
world_y = 高さ（ジャンプ用に維持。0 = 地面）

screen_x = world_x - cam_x
screen_y = world_z - cam_y - world_y
```

- world Y（高さ軸）は ADR-0017 の予約を踏襲して残す（jump / 打ち上げ）。
- **Z の向きが ADR-0017 から反転する**（Z+ = 奥 → Z+ = 手前 = 画像下）。これに伴い:
  - **描画順 sort は world Z 昇順**（奥=Z 小を先、手前=Z 大を後＝上に描く）。
  - **movement の Up/Down 入力の符号を反転**（Up で奥=Z 減、Down で手前=Z 増）。
  - **Area の不変条件が `near_z <= far_z` → `near_z >= far_z`**（near=手前=画像下=大、far=奥=画像上=小）。`Contains` の補間式自体は near>far でも代数的に成立し、範囲ゲートだけ反転する。
- 背景描画は `Translate(-cam_x, -cam_y)`（画像ピクセル原点を world / screen 原点に合わせる）。
- camera の `camera_start_x` / `camera_start_y` は「視界左上隅の画像ピクセル座標」を意味する（X 追従 / Y 固定の構造は維持）。
- 投影は引き続き `shared/render/projection.go::Projector` の 1 箇所に閉じる（hit 判定 / box overlay / sprite 描画が共有）。

## Alternatives Considered

- **Z 符号と 8px オフセットだけをピンポイント修正し、投影パラメータは残す**
  - 変更は最小。`ground_screen_y` / `z_scale` による Level 単位の調整余地も残る。
  - 座標系が 3 段ある根本問題（直感的に追えない）が解決しない。editor / engine の符号を再び取り違える余地が残り、同種バグの温床になる。

- **運用で base 画像高さ = 解像度高さに揃える**
  - コード変更不要で 8px オフセットだけは消える（偶然 `scrH - bgH = 0` になる）。
  - 横方向は明らかに画像 > 解像度（スクロール）なのに縦だけ一致を強制するのは非対称で、Z 符号反転バグも残る。対症療法。

- **ADR-0017 の「Z+ = 奥」を維持したまま editor 側だけ符号を直す**
  - editor の `+ world_z` を `- world_z` にすれば符号は揃う。
  - 「画像ピクセル = world」一本化のメリット（投影パラメータ廃止、直感性）が得られない。`ground_screen_y` / `z_scale` の二重管理が残る。

## Consequences

**得られたもの**

- 「editor の Canvas で見えている base 画像上の位置 = world 座標 = （camera を引く前の）画面位置」になり、座標の対応が 1 対 1 で追える。
- Level モデルから `ground_screen_y` / `z_scale` が消え、YAML / struct / 編集 UI が単純化する（コード総量は減る）。
- editor / engine の Z 符号不一致バグと背景 8px オフセットが、投影式を作り直す過程で同時に解消する。
- 投影が単純な平行移動になり、`Projector` を介する原則は維持される（hit 判定 / 描画が引き続き同じ式を共有）。

**支払うコスト / 注意点**

- `z_scale` による Level 単位の「奥行きの効きの強さ」調整、`ground_screen_y` による地面ラインのパラメータ化を失う（必要になれば再導入可能。現状全 Level で z_scale=1.0 のため実害なし）。
- **Z の向きが ADR-0017 から反転**するため、既存 Level データの Z 値を移行する必要がある。旧 editor が `image_y = ground_screen_y + z` で表示していたので、`new_z = old_z + ground_screen_y` で変換すれば editor の見た目は不変のまま engine が追従する。Area は `near_z >= far_z` を満たすよう near/far ラベルと対応 X をスワップする。
- 描画順 sort・movement の Up/Down・Area 不変条件・loader バリデーションの符号がすべて反転するので、関連テストも合わせて更新する。

**関連 ADR**

- ADR-0017: 本 ADR が supersede（world 3 軸の枠組みと world Y=高さは踏襲、Z の向きと投影パラメータを変更）。
- ADR-0022: Area = 1 辺平行台形 + OR 合成（判断は有効。near/far の Z 大小関係だけ本 ADR で反転）。
- ADR-0021: world 3D AABB hit 判定（Z 対称区間なので符号反転の影響を受けない）。
- ADR-0011: filesystem YAML を一次ストレージとする。
