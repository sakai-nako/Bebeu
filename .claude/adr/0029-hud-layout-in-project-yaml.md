# ADR-0029: HUD レイアウトは Project YAML に持つ

## Status

Accepted (2026-06-25 に Player HP bar の見た目スキーマを拡張 / 同日 PlayerHpRing を実装)

## Context

Gameplay 中の HUD (HP バー、将来はスコア・タイマー・残機など) を実装するにあたり、
配置 (anchor / offset / size) と要素の種類を **どこに格納するか** を決める必要が出た。

候補:

1. **engine 内 hardcode** — Rust 定数で各 HUD 要素の座標を持つ
2. **`workspace/data/hud.yml` 単独ファイル** — workspace に HUD 専用 YAML を新設する
3. **`workspace/data/projects/{name}.yml` に同居** — 既存の Project YAML に `hud:` セクションを足す

Project は本リポジトリで「engine 起動時に何を読み込むか」を束ねる preset 単位として
すでに機能している ([model.rs](../../packages/engine/src/entities/project/model.rs):
resolution / players / opponents / levels)。1 workspace に複数 Project を並べ、
`--project <name>` で切り替える運用。

HUD レイアウトはステージや出すキャラに依存して変えたくなりやすい (チュートリアル用と
本編用で HP バー位置を変える、デモ動画用に HUD を消す、など)。逆に「workspace 全体で
共通の唯一の HUD」になることは想定しにくい。

## Decision

**HUD レイアウトは Project YAML の `hud:` セクションとして持つ** (候補 3)。
要素は配列 `elements:` で、要素ごとに `kind` を判別キーに、kind ごとの設定を flat に並べる
internally-tagged enum (`#[serde(tag = "kind", rename_all = "snake_case")]`) を採用する。

```yaml
# workspace/data/projects/main.yml
resolution: { width: 384, height: 216 }
players: [hero]
opponents: [enemy]
levels: [training]
hud:
  elements:
    - kind: player_hp_bar
      anchor: top_left
      offset: { x: 16.0, "y": 16.0 }
      size:   { w: 120.0, h: 8.0 }
      frame:  { thickness: 1.0, color: "#000000" }
      bg_color: "#00000099"
      fg_color: "#e62626"
      fill_direction: left_to_right
      gauge_step: { fixed_count: 1 }
      gauge_gap: 0.0
```

スキーマ:

- `kind` は snake_case の enum タグ。Rust 側は `#[serde(tag = "kind", rename_all = "snake_case")]`
  の internally-tagged enum + variant ごとの Config struct (例: `PlayerHpBarConfig`)。要素を
  増やすときは variant と Config を 1 ペア追加する。
- `anchor` は画面 9 アンカー (`top_left` / `top` / `top_right` / `left` / `center` /
  `right` / `bottom_left` / `bottom` / `bottom_right`)。
- `offset` は anchor からのピクセルオフセット (画面感覚: X 右が正、Y 下が正)。
- `size` は要素の **外形 bbox** サイズ。`frame.thickness` 分は size の内側に食い込む
  (= 枠を太くしても全体の幅は変わらない)。
- 要素自体の anchor は当面 **top-left 固定** (offset 位置に要素の左上隅が来る)。

YAML key 名で `y` は YAML 1.1 の真偽値 alias なので **必ずクォートする** (`"y": 16.0`)。
これは saphyr parser の挙動。

### Player HP bar の見た目スキーマ (2026-06-25 拡張)

`PlayerHpBarConfig` の追加フィールド:

- `frame: { thickness, color }` — 外枠の太さ (`size` の内側に食い込む) と色。`thickness: 0` で枠なし。
- `bg_color` / `fg_color` — 内側の背景色 / 前景色 (ゲージ塗り)。**ゲージ本数が複数でも 1 色ずつ**。
  per-gauge 色は当面サポートしない (beat-em-up 系で多数派の見た目 + YAML/UI 複雑度抑制)。
- `fill_direction` — `left_to_right` / `right_to_left` / `top_to_bottom` / `bottom_to_top`。
  縦バーは TTB / BTT。**円型 (radial) は別 variant** として将来追加する (角度・半径系の
  別パラメータが必要なため、bar variant に optional field を生やすより独立した方が clean)。
- `gauge_step` — 1 本の HP バーを何本のゲージで見せるかの規則。タグ付き enum で 2 通り:
  - `{ fixed_count: n }`: 常に n 等分 (max HP 非依存)
  - `{ per_unit: n }`: 1 ゲージ = n HP として `ceil(max_hp / n)` 本に分け、最後のゲージは
    余り HP 分だけ持つ。視覚的には全ゲージ等幅で、最後のゲージだけ早く満タンになる。
- `gauge_gap` — 複数ゲージ間の隙間 (px)。隙間部分は bg_color が透ける描画 (枠と同色で
  埋める需要が出たら別 field を後付け)。

色は `#RRGGBB` または `#RRGGBBAA` の hex 文字列で受ける (YAML 親和性 + editor の
color picker 直結。alpha は `<input type="number" min=0 max=1>` で別 input)。

ゲージの減り方:
- `fill_direction` の終端側のゲージから減る (LTR なら一番右のゲージが最初に欠ける)
- 1 ゲージ内は smooth fill (常にフル or 空のステップ表示はしない)
- ダメージ遅延 (赤帯が一拍遅れて減る) / low-HP の色変化はまだ実装しない (`low_hp_threshold`
  と `low_hp_color` を別 field として後付けする想定)

engine と editor で別々に同じ struct を持つ ([ADR-0001] FSD の編集独立性に従う)。
描画 system は `features/hud/` slice (engine 側 segment 無し) に集約し、要素 kind ごとに
1 つの spawn + update system を生やす。

### Player HP ring の見た目スキーマ (2026-06-25 実装)

`player_hp_ring` kind を新 variant `HudElement::PlayerHpRing(PlayerHpRingConfig)` として
追加した。HP bar と並べて使え、両方を同じ Project に置ける。

```yaml
- kind: player_hp_ring
  anchor: top_left
  offset: { x: 16.0, "y": 16.0 }
  size:   { w: 48.0, h: 48.0 }   # 外接 bbox。半径 = min(w, h) / 2
  frame:  { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#e62626"
  start_angle: 15.0     # 度。12 時方向 = 0°
  sweep_extent: 330.0   # 度。360 で完全な円、それ以下は部分円弧 (start ± sweep)
  ring_thickness: 8.0   # 太さ (px)。0 で扇形 (pie)、center まで埋まる
  direction: clockwise  # clockwise | counter_clockwise
  gauge_step: { fixed_count: 1 }
  gauge_gap: 0.0        # **度単位**の隙間 (bar の px 単位とは異なる)
```

スキーマ判断 (HP bar との対比):

- **角度系の基準** — `start_angle` は 12 時方向を 0° とし、`direction` の向きに `sweep_extent`
  度ぶん描画する。時計や格ゲーゲージの直感に合わせる。例: `start=15, sweep=330, clockwise`
  → 12 時から時計回りに +15° の位置から開始し、12 時から -15° の位置で止まる (上方向に 30°
  の隙間)。
- **`gauge_gap` は度単位** — px 単位だと半径によって見た目の隙間が変わるため。HP bar の
  `gauge_gap` (px) とは意味が違う点に注意。
- **減る向きは bar と同じ規約** — `direction` の終端側 segment から欠ける。clockwise なら
  時計回りに最後の segment (= 反時計側) から消える。
- **`ring_thickness = 0` で扇形 (pie)** — エラーにせず連続的な値として許容。
- **`frame` の意味** — `ring_thickness + 2 * frame.thickness` の太さの annular sector を裏側
  に置く。半径方向に内外両側へ枠が出る。さらに `sweep_extent < 360°` のときは両端の弦に
  細長い sprite を 2 枚 radial 方向に回転配置し、弦にも枠を出す (`= 360°` のときは両端が
  重なるため抑制)。弦 sprite の長さ = `ring_thickness + 2 * frame.thickness`、厚み =
  `frame.thickness`。
- **中心が透ける** — annular sector mesh は中心領域には頂点を持たないため、`ring_thickness
  < radius` なら ring 内側に背後の sprite (例: プレイヤーアイコン) が透ける。z は `HudRoot`
  配下の局所 z で frame=0.0 / bg=0.1 / fg=0.2 のレイヤを使う。

実装上の選択:

- リング mesh は自前で triangle list を組む (1° あたり 1 step)。Bevy 0.18 の `Mesh2d` +
  `MeshMaterial2d<ColorMaterial>` で描画。`bevy_mesh::primitives::Annulus` は完全な閉じ
  リングしか作れないので、部分円弧には使えない。
- HP 変化に伴う fg mesh の差し替えは `Changed<HitPoints>` で gating し、`Assets<Mesh>::insert`
  で in-place 更新する (handle を再 allocate しない)。

## Alternatives Considered

- **engine hardcode (候補 1)**: ゲームデザイナーが engine リポジトリのコードを触らないと
  HUD を動かせない。1 workspace に複数 project を並べる運用と相性が悪い。
- **`workspace/data/hud.yml` 単独 (候補 2)**: workspace に唯一の HUD 設定を置く形。
  シンプルだが「project ごとに HUD を変える」需要に対して、結局 project から hud.yml への
  参照を持つか project ごとに `hud-{project}.yml` を増やすかになり、最終的に候補 3 と同形に
  収束する。先に Project YAML 内に置く方が階層が浅い。
- **per-gauge color**: 複数ゲージ時にゲージごとに色を持たせる案。beat-em-up 系では稀
  (Streets of Rage / Final Fight 系は全ゲージ同色)、editor UI が list 編集化して複雑度が
  上がる割に表現力の伸びが少ないため不採用。残量帯で色を変える需要は `low_hp_threshold`
  と `low_hp_color` で後付け可能。
- **横/縦/円型を 1 variant に統一**: `fill_direction` enum で 4 種類とも扱う案。円型は
  角度 (`start_angle` / `sweep_extent`) / 半径 (内径外径) / 回転方向など bar とは別系統の
  パラメータが必要で、1 variant に optional field を増やすと無関係 field の海になる。
  bar (横/縦) と radial (円) を別 kind に分ける方が clean。

## Consequences

**得られたもの**

- 1 つの project YAML を見れば、起動時に何が画面に出るか (キャラ + ステージ + HUD) が
  一望できる。editor 側でも `ProjectDetail` 画面で全部編集できる。
- 要素追加が enum variant + Config struct + 1 ペアの spawn/update system だけで済む。
- `#[serde(default)]` で `hud:` セクションを省略しても既存 project は壊れない (空 HUD)。
- 枠 / 色 / 方向 / multi-gauge まで YAML 駆動で調整できる (= デザイナー単独で見た目変更可)。

**支払うコスト**

- Project YAML が大きくなる。要素数が増えた場合に 1 ファイルが見づらくなる可能性がある
  (現状は Player HP bar 1 個なので問題なし)。
- Project struct のフィールドが増える → editor / engine 双方で struct 同期が必要 (ADR-0011
  方針通り、これは許容)。
- 円型バー / ダメージ遅延 / low-HP 色変化 / 数値表示 (text font) を入れたいときに、また
  スキーマ拡張が要る。MVP + 今回拡張までは枠と複数ゲージで十分とした。

**今後の拡張余地**

- `EnemyHpBar { target_index: usize }` 等、target を持つ HUD 要素 → 同じ internally-tagged
  enum の別 variant で素直に追加できる ([`PlayerHpBarConfig`] と同じ shape にできる)。
- `PlayerIcon` (ring 内中央への顔アイコン等) → 別 variant。ring とは独立した HUD 要素として、
  anchor + offset + size で位置を合わせて中央に配置する想定。
- editor のライブプレビュー (HUD 配置を視覚的に確認) は別の段階で検討。今回も数値入力 +
  color picker のみ。
- low-HP color flash / damage delay は `low_hp_threshold` + `low_hp_color` + `delay_color`
  を後付けで足す形を予約。

## 関連

- [ADR-0001](0001-adopt-feature-sliced-design.md): FSD レイヤ規約。engine の `features/hud/`
  は segment 無し、editor の `entities/project/model.rs` 内に Hud 型を同居 (サブスライス禁止)
- [ADR-0011](0011-filesystem-yaml-as-primary-storage.md): YAML を primary storage とする方針
- [ADR-0016](0016-engine-config-hybrid-placement.md): 「キャラ固有 ≒ workspace data、engine 横断 ≒
  engine config」の二分法。HUD レイアウトは Project 固有なので workspace data 側に置く
- `packages/engine/src/features/hud.rs`: 描画 system 実装
- `packages/editor-desktop/src/features/hud/ui/edit_hud_layout.rs`: 編集フォーム
- `packages/engine/src/entities/project/model.rs`: `PlayerHpBarConfig` / `HexColor` /
  `FillDirection` / `GaugeStep` 等の型定義

[ADR-0001]: 0001-adopt-feature-sliced-design.md
