# ADR-0017: world 軸 (X / Y=高さ / Z=奥行き) と 2.5D 投影

## Status

Superseded by [ADR-0023](0023-image-pixel-world-screen-unification.md)

> world 3 軸の枠組みと world Y=高さは ADR-0023 でも踏襲されるが、投影パラメータ
> (`GroundScreenY` / `ZScale`) は廃止され、Z の向きが反転した（Z+ = 手前）。
> 以下は歴史的記録として残す。

## Context

Stage 2 で 2 体目スポーン + Up/Down 入力による奥行き移動を入れる時点で、entity の位置を保持する座標系を決める必要があった。要件は次の通り:

- Beat 'em up 慣例の俯瞰視点で「奥/手前への歩き」が表現できる
- 既存の 2D スプライトをそのまま使える (3D モデルは導入しない)
- editor (`packages/editor/`) の box 編集座標系と engine の hit 判定座標系を同じ規約で動かしたい (editor / engine FSD 対称運用)
- jump (Y 軸) は Stage 4 以降で導入する余地を残す

候補の取り方は次の 3 つあった:

1. screen 座標 (X, Y) のみ。奥行きは「キャラ画像を縦方向に動かす」ことで暗黙表現
2. world (X, Y) + 投影なし、Y を screen 上の絶対位置として使う (=「Y 軸が高さ」と「Y 軸が奥行き」を兼ねる)
3. world 3 軸 (X, Y=高さ, Z=奥行き) + 投影で screen 2D に落とす

## Decision

**world は 3 軸 (X / Y=高さ / Z=奥行き) で持ち、`shared/render/projection.go::Projector` が唯一の world→screen 変換点になる。**

```
screen_x = world_x
screen_y = level.GroundScreenY - world_y - world_z * level.ZScale
```

- world Y は + で上昇 (jump 用に予約)。Stage 2 では 0 固定
- world Z は + で奥、- で手前。Up/Down 入力で増減
- 投影パラメータ (`GroundScreenY`, `ZScale`) は Level YAML に持たせ、Level 単位で地面ラインと奥行きの強さを差し替えられるようにする
- entity の格納フィールドは `movement.State{PosX, PosY, PosZ}` (float64)
- 描画順は `Draw` で毎 tick `world_z` 降順 (奥 → 手前) に sort

## Alternatives Considered

- **screen 2D のみ (`PosX`, `PosY` を直接スクリーン Y として扱う)**:
    - 実装は最も単純。Up/Down で `PosY` を直接動かせば奥行き表現になる
    - 問題: jump (高さ) を後から入れたとき「Y 軸が `画面 Y - 高さ - 奥行き*係数` を兼ねる」状態を扱う必要があり、結局この ADR で決めた式が暗黙化されるだけ。Z を分離せず後から足すと movement.State の意味が壊れる
    - hit 判定で「奥行きが離れていれば当たりにくい」を表現するには world Z 相当の値が結局必要

- **world (X, Y=奥行き) + 高さは別変数**:
    - Y 軸の意味を「高さ」にせず「奥行き」にするパターン。Beat 'em up 業界では実装によりまちまち
    - editor 側では box の `top_left[1]` / `bottom_right[1]` が「画像の Y (= 高さ)」を意味する以上、world Y も「高さ」に揃えたほうが両者の頭の切り替えが少ない
    - 「Y=高さ, Z=奥行き」のほうが 3D 業界の慣例に近く、Stage 4 で AI / 攻撃ロジックを書く時の共通言語として安全

- **完全な 3D 透視投影 (z による x スケール変化込み)**:
    - スプライトの拡大縮小やパースが付くため、奥のキャラが小さく描かれる演出ができる
    - 2D 素材をそのまま使う前提と相性が悪い (奥のキャラが小さくなると box editor で詰めた当たり判定とずれる)
    - Beat 'em up 慣例ではアフィン投影 (奥行きで縦位置だけずらす) が多数派

## Consequences

**得られたもの**

- world ↔ screen 変換が `Projector` の 1 関数に集約され、hit 判定 (`features/attack`) / box overlay (`widgets/hitboxoverlay`) / sprite 描画 (`shared/render/frame.go`) が同じ式を共有する
- jump (Y 軸) を将来導入しても `world_y` を non-zero にすれば screen_y が `- world_y` だけ動くので、movement の API 互換は壊れない
- Level YAML で `ground_screen_y` と `z_scale` を変えるだけで、地面ラインと奥行きの強さが Level 単位で切り替わる
- 描画順 (奥 → 手前) と hit 判定 (screen AABB) が同じ world Z から導出されるため、「描画上手前のキャラの足元と奥のキャラの腰が重なる」と「screen Y が近い → hit 判定で当たる」が物理的に整合する → ADR-0018 の前提

**支払うコスト / 注意点**

- world ↔ screen を毎フレーム計算するコストが増える (描画と hit 判定で `entityWorld()` を 2 回呼ぶ等)。entity 数が 2〜数体のうちは無視できる
- Level YAML 必須項目が増える (`ground_screen_y`, `z_scale`, `bounds.min_z` / `max_z`)。loader は `Default` フォールバックを持つので欠損しても落ちない
- editor 側の box 座標は依然「sprite 画像 pixel」基準なので、world Z は editor から見えない。editor の box 編集と engine の hit 判定の橋渡しは「sprite 描画位置 = `Projector.ToScreen(entity.world)`」という暗黙の合意で動いている (= editor 側の Pivot Offset の式が engine 側 `LayerOrigin` と一致することで保たれる、ADR-0007 関連)
- camera を Stage 3+ で導入する場合 `screen_x = world_x - cam.X` に変わる。`Projector` を経由する原則を維持しておけば、変更点はこの構造体だけに閉じる

**今後の拡張余地**

- 透視投影 (奥のキャラを縮小) を入れたくなった場合は `Projector` に scale 係数を追加し、描画と hit 判定の双方で同じ scale を使う形で拡張できる
- world Y (高さ) を活用する jump / 飛び道具は、`movement.State.PosY` を更新するだけで描画と hit 判定の両方が自然に追従する
