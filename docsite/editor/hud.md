# HUD レイアウト

ゲームプレイ中の HUD (HP バー、HP リング、敵 HP バー) は Project YAML の `hud:` セクションで定義する。要素は配列 `elements:` で並べ、要素ごとに `kind:` を判別キーに持つ internally-tagged enum。

エディタ上では Project 詳細ページの「HUD レイアウト」カードで GUI 編集できる。要素の追加・削除・並べ替え・数値 / 色 / dropdown のフォームから全フィールドを設定する。

## 要素の種類

| `kind` | 概要 | アンカー | ADR |
|---|---|---|---|
| `player_hp_bar` | Player の HP を表示するバー (横 / 縦) | screen | ADR-0029 / ADR-0030 |
| `player_hp_ring` | Player の HP を表示する円弧 (annular sector) | screen | ADR-0029 |
| `enemy_hp_bar` | Enemy の HP を表示するバー。target は engagement / tag / nth | screen | ADR-0031 |
| `enemy_overhead_hp_bar` | Enemy entity の頭上に追従する小さいバー | world | ADR-0032 |

`player_hp_bar` / `player_hp_ring` / `enemy_hp_bar` は **screen-anchored**: 画面 9 アンカー (`top_left` / `top` / `top_right` / `left` / `center` / `right` / `bottom_left` / `bottom` / `bottom_right`) からの offset で配置する。`enemy_overhead_hp_bar` のみ **world-anchored** で、Enemy entity の Transform に追従する。

## 共通フィールド (screen-anchored 系)

| フィールド | 単位 / 既定値 | 役割 |
|---|---|---|
| `id` | 文字列 / 省略可 | 他要素から `anchor_to.id` で参照されるための識別子 |
| `anchor` | 9 アンカー / `top_left` | 画面上の基準点 |
| `anchor_to` | `{ id, edge }` / 省略可 | **指定時は `anchor` が無視される**。他要素の `edge` を基準点に取る (ADR-0031) |
| `offset` | `{ x, "y" }` (px) | 基準点からのピクセルオフセット (X 右が正、Y 下が正) |
| `size` | `{ w, h }` (px) | 要素の外形 bbox |
| `frame` | `{ thickness, color }` | 外枠の太さ (size の内側に食い込む) と色。`thickness: 0` で枠なし |
| `bg_color` / `fg_color` | `#RRGGBB` / `#RRGGBBAA` | 内側の背景色 / 前景色 (ゲージ塗り) |
| `fill_direction` | enum / `left_to_right` | ゲージが減っていく向き: `left_to_right` / `right_to_left` / `top_to_bottom` / `bottom_to_top` |

> YAML の key `y` は YAML 1.1 の真偽値 alias なので、`offset` 内では **必ずクォートする** (`"y": 16.0`)。saphyr parser がそのまま受け取る。

## `player_hp_bar`

Player の HP を 1 本のバーで表示。size の幅 / 高さは外形 bbox で、frame と内側ゲージ描画領域はその内側。

`fill_direction` の終端側のゲージから減る (`left_to_right` なら一番右が最初に欠ける)。1 ゲージ内は smooth fill。

追加フィールド:

| フィールド | 既定値 | 役割 |
|---|---|---|
| `target` | `p1` | 表示対象の `PlayerId` (`p1` / `p2` / `p3` / `p4`)。該当 Player が居ない要素は spawn 時に warn を出して skip される (ADR-0030) |
| `gauge_step` | `{ fixed_count: 1 }` | 1 本の HP バーを複数ゲージに分ける規則 (下記) |
| `gauge_gap` | `0.0` | 複数ゲージ間の隙間 (px)。隙間部分は bg_color が透ける |

`gauge_step` は tag つき enum で 2 通り:

- `{ fixed_count: n }`: max HP に依らず常に n 等分する
- `{ per_unit: n }`: 1 ゲージ = n HP として `ceil(max_hp / n)` 本に分ける。最後のゲージは余り HP 分だけ持ち、視覚的には全ゲージ等幅で最後だけ早く満タンになる

```yaml
- kind: player_hp_bar
  id: p1_hp
  target: p1
  anchor: top_left
  offset: { x: 16.0, "y": 16.0 }
  size: { w: 120.0, h: 8.0 }
  frame: { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#e62626"
  fill_direction: left_to_right
  gauge_step: { fixed_count: 1 }
  gauge_gap: 0.0
```

## `player_hp_ring`

Player の HP を annular sector (円環) で表示。`size` は外接 bbox、半径 = `min(w, h) / 2`。

| フィールド | 既定値 | 役割 |
|---|---|---|
| `target` | `p1` | 表示対象の `PlayerId` |
| `start_angle` | `0.0` | 開始角 (度)。12 時方向 = 0° |
| `sweep_extent` | `360.0` | 描画する円弧の角度 (度)。`360` で完全な円、それ以下は部分円弧 |
| `ring_thickness` | `6.0` | リングの太さ (px)。`0` で扇形 (pie) になり中心まで埋まる |
| `direction` | `clockwise` | リングを描画する回転方向: `clockwise` / `counter_clockwise` |
| `gauge_step` / `gauge_gap` | bar と同じ | ただし `gauge_gap` は **度単位** (px ではない、半径で見た目が変わらないようにするため) |

`fill_direction` は使わず、減る向きは `direction` の終端側 segment から欠ける。`ring_thickness < radius` なら中心が透けるので、ring 内側に別の HUD 要素 (アイコン等) を重ねられる。

```yaml
- kind: player_hp_ring
  target: p1
  anchor: top_left
  offset: { x: 16.0, "y": 16.0 }
  size: { w: 48.0, h: 48.0 }
  frame: { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#e62626"
  start_angle: 15.0
  sweep_extent: 330.0
  ring_thickness: 8.0
  direction: clockwise
```

## `enemy_hp_bar`

screen-anchored な Enemy HP bar。target が時間で変わる用途を想定しており、Phase 2 では **常に単一 gauge** (`gauge_step` / `gauge_gap` は schema 互換性のため残しているが engine 側で無視される)。

`target` は externally-tagged enum で 3 種類:

| YAML | 意味 |
|---|---|
| `target: { last_engaged_by: p1 }` | 指定 Player が直近で殴った Enemy を映す。攻撃が成立するたびに切り替わる |
| `target: { tag: boss }` | Character YAML の `tag` フィールドが一致する Enemy を映す。boss 用途 |
| `target: { nth_enemy: 0 }` | spawn 順 N 番目の Enemy を映す。debug 用 |

該当 Enemy が居ない瞬間は `Visibility::Hidden` で frame / bg / gauge ごと消える。

```yaml
# P1 が直近で殴った enemy の HP bar を、id "p1_hp" の Player HP bar の真下に置く
- kind: enemy_hp_bar
  target: { last_engaged_by: p1 }
  anchor_to:
    id: p1_hp
    edge: bottom_left
  offset: { x: 0.0, "y": 4.0 }
  size: { w: 120.0, h: 6.0 }
  frame: { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#f2d82c"
  fill_direction: left_to_right
```

`tag` を使うには Character 側に `tag: boss` のような文字列を書いておく (engine 起動時に Enemy entity の `EnemyTag` component に乗る)。

## `enemy_overhead_hp_bar`

world-anchored で、画面上の **各 Enemy entity の頭上**に追従する小さいバー。screen anchor 系のフィールド (`anchor` / `anchor_to` / `id` / `offset.x`) は意味を持たない。

| フィールド | 既定値 | 役割 |
|---|---|---|
| `tag_filter` | 省略 (全 Enemy) | 指定時は Character の `tag` が一致する Enemy にだけ attach |
| `size` | `{ w: 28.0, h: 3.0 }` | バーの外形 bbox |
| `frame` / `bg_color` / `fg_color` / `fill_direction` | screen 系と同じ | |
| `vertical_anchor` | `image_top` | Y 基準点 (下表) |
| `offset_y` | `4.0` | 基準点からの Y オフセット (bevy Y、+ が上) |

`vertical_anchor` の 3 値:

| 値 | 基準点 | 追従挙動 |
|---|---|---|
| `origin` | Enemy entity の Transform 原点 (= sprite anchor、通常足元) | 固定 |
| `image_top` (既定) | 現フレームの sprite **上端** | **毎 frame 追従** (jump で sprite が伸びれば bar も上にスライド) |
| `image_bottom` | 現フレームの sprite **下端** | **毎 frame 追従** |

```yaml
# 全 enemy の頭上に小さい黄色バー
- kind: enemy_overhead_hp_bar
  size: { w: 28.0, h: 4.0 }
  frame: { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#f2d82c"
  vertical_anchor: image_top
  offset_y: 4.0
  fill_direction: left_to_right
```

`Added<Enemy>` 検出で per-enemy spawn され、Enemy が despawn されると Bevy hierarchy で連鎖破棄される。

## HUD 要素間 anchor (`anchor_to`)

screen-anchored 系の要素は、`anchor` の代わりに他要素を基準点に取れる:

```yaml
- kind: player_hp_bar
  id: p1_hp                          # 他要素から参照される識別子
  anchor: top_left
  offset: { x: 16.0, "y": 16.0 }
  size: { w: 120.0, h: 8.0 }
- kind: enemy_hp_bar
  target: { last_engaged_by: p1 }
  anchor_to:                          # P1 HP bar の bottom-left を基準
    id: p1_hp
    edge: bottom_left
  offset: { x: 0.0, "y": 4.0 }
  size: { w: 120.0, h: 6.0 }
```

参照は **前方向のみ** (= YAML 内で親が先に書かれている)。未解決 id は warn 出して要素 skip。実体は Bevy の Transform hierarchy なので、親が camera に追従していれば連鎖で追従する。

## 完全な YAML 例

`sample-projects/minimal/data/projects/main.yml` の HUD セクション:

```yaml
hud:
  elements:
    - kind: player_hp_bar
      id: p1_hp
      target: p1
      anchor: top_left
      offset: { x: 16.0, "y": 16.0 }
      size: { w: 120.0, h: 8.0 }
      frame: { thickness: 1.0, color: "#000000" }
      bg_color: "#00000099"
      fg_color: "#e62626"
      fill_direction: left_to_right
      gauge_step: { fixed_count: 1 }
      gauge_gap: 0.0
    - kind: enemy_hp_bar
      target: { last_engaged_by: p1 }
      anchor_to: { id: p1_hp, edge: bottom_left }
      offset: { x: 0.0, "y": 4.0 }
      size: { w: 120.0, h: 6.0 }
      frame: { thickness: 1.0, color: "#000000" }
      bg_color: "#00000099"
      fg_color: "#f2d82c"
      fill_direction: left_to_right
    - kind: enemy_overhead_hp_bar
      size: { w: 28.0, h: 4.0 }
      frame: { thickness: 1.0, color: "#000000" }
      bg_color: "#00000099"
      fg_color: "#f2d82c"
      vertical_anchor: image_top
      offset_y: 4.0
      fill_direction: left_to_right
```
