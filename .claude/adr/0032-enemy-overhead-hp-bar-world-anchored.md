# ADR-0032: world-anchored な Enemy 頭上 HP bar

## Status

Accepted (2026-06-25 に Phase 3 として `enemy_overhead_hp_bar` kind を追加 /
2026-06-25 に Y 基準を sprite top/bottom 相対に拡張)

## Context

ADR-0031 で導入した `enemy_hp_bar` (screen-anchored) と engagement-link で 3 用途のうち
(1) engagement-link と (2) boss bottom-fixed を 1 kind で扱った。残った (3) 頭上追従 は描画
機構が違うため別 kind として扱う設計を予約してあった。

頭上追従 (overhead) の特徴:

- **per-enemy spawn**: 1 設定 から **画面上の N 個** (= 現在の Enemy 数) ぶん bar が描画される
- **world-anchored**: Enemy entity の Transform に追従し、camera スクロールとは独立して
  Enemy 上に貼り付く
- **spawn / despawn は Enemy の生成 / 消滅で連動**: `Added<Enemy>` 検出と Bevy hierarchy 自動破棄
- **target という概念がない**: tag_filter で attach 対象を絞るだけ (全 enemy or 特定 tag)
- **anchor / offset / id / anchor_to は意味を持たない**: screen anchor 系の field が全部不要

これを `enemy_hp_bar` に optional field で重ねると無関係 field の海になるので、独立した
`enemy_overhead_hp_bar` kind に分けた。

## Decision

### 1 個目の world-anchored HUD kind を追加

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HudElement {
    PlayerHpBar(PlayerHpBarConfig),
    PlayerHpRing(PlayerHpRingConfig),
    EnemyHpBar(EnemyHpBarConfig),
    EnemyOverheadHpBar(EnemyOverheadHpBarConfig),  // 新規
}

pub struct EnemyOverheadHpBarConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_filter: Option<String>,
    pub size: HudSize,
    pub frame: HudFrame,
    pub bg_color: HexColor,
    pub fg_color: HexColor,
    pub world_offset_y: f32,        // bevy 局所 Y、Enemy origin (足元) から上方向
    pub fill_direction: FillDirection,
}
```

```yaml
hud:
  elements:
    - kind: enemy_overhead_hp_bar     # tag_filter 省略 = 全 enemy
      size: { w: 28.0, h: 3.0 }
      world_offset_y: 40.0
      fill_direction: left_to_right
    - kind: enemy_overhead_hp_bar     # boss 限定 (Character.tag == "boss")
      tag_filter: boss
      size: { w: 64.0, h: 5.0 }
      world_offset_y: 56.0
      frame: { thickness: 1.0, color: "#fff" }
```

### `HudElement::is_screen_anchored()` で経路を分ける

`spawn_hud` の screen-anchor 経路は `is_screen_anchored() == true` の要素だけ処理。
`EnemyOverheadHpBar` は `false` を返し、別 system が扱う。

screen-anchor 経路で呼ばれる `anchor()` / `anchor_to()` / `offset()` / `id()` は、overhead
variant では default 値を返すスタブにし、match の網羅性のため形だけ残す。実際の値が
使われることはない。

### `Added<Enemy>` で per-enemy spawn

```rust
fn spawn_enemy_overhead_hp_bars(
    mut commands: Commands,
    project: Option<Res<Project>>,
    new_enemies: Query<(Entity, Option<&EnemyTag>), Added<Enemy>>,
) {
    for (enemy_entity, tag) in &new_enemies {
        for cfg in overhead_configs(&project) {
            if let Some(filter) = &cfg.tag_filter {
                if tag.map_or(true, |t| &t.0 != filter) { continue; }
            }
            spawn_overhead_bar(&mut commands, enemy_entity, cfg);
        }
    }
}
```

各 Enemy entity の **子** として bar root entity を spawn する (`ChildOf(enemy_entity)`)。
Bevy の Transform hierarchy で Enemy が動けば bar も追従、Enemy が despawn されれば bar も
自動 cascade 破棄。HudPlugin 側で OnExit(Battle) の `despawn_hud` で `HudRoot` 全部消すので、
シーン遷移時の cleanup も既存 path を流用できる。

### 配置: vertical_anchor + offset_y で 3 種類の基準点

初版は `world_offset_y` 1 個で「Enemy origin から上にこの px」の固定だったが、これだとキャラ
身長に応じた project ごとの調整が必須で、姿勢で sprite が伸縮する局面 (jump で縦に伸びる等)
にも追従できない。後付けで `vertical_anchor` enum + `offset_y` の 2 field に分解した:

| `vertical_anchor` | 基準点 | 追従挙動 |
|------------------|-------|---------|
| `origin` | Enemy entity の Transform 原点 (sprite anchor、通常足元) | 固定 |
| `image_top` (default) | 現フレームの sprite **上端** | **毎 frame 追従** |
| `image_bottom` | 現フレームの sprite **下端** | **毎 frame 追従** |

`offset_y` は基準点から bevy Y 方向の追加オフセット (+ が上、− が下)。default は
`image_top + 4` で「画像上端の 4 px 上」になり、キャラ身長に依らず「頭の少し上」に出る。

実装は `FrameRender.image_dims` (画像 [w, h] px) と `sprite_pivot[1]` から:

- `image_top` 位置 = bevy 局所 Y で `+pivot_y` (画像上端は pivot から +Y 方向に pivot_y px)
- `image_bottom` 位置 = bevy 局所 Y で `-(image_height - pivot_y)`
- `origin` 位置 = bevy 局所 Y で `0`

`update_enemy_overhead_hp_bar` system が **毎 frame** これを再計算し、bar root の
`Transform.translation.y` を更新する。jump で sprite が伸びれば bar も上にスライドし、
hit pose で縮めば下にスライドする。

z は 10 (= キャラ sprite の前、screen HUD の HUD_Z=100 の後ろ) で、camera の near/far に
収まる範囲。

### Anchor 規約: Sprite 中央 + LeftToRight gauge は端寄せ

bar の root は Enemy origin から (0, world_offset_y) に置き、Sprite は **bbox 中央** に
描く (= `Anchor::CENTER`)。Enemy が画面上で左右に動いても bar は Enemy の真上に来る。

gauge sprite は fill_direction に応じて端寄せ (`Anchor::CENTER_LEFT` / `CENTER_RIGHT`) し、
ratio に応じて `custom_size.x` を縮める (= 単一 gauge、`enemy_hp_bar` と同じ規約)。

### Update は per-bar query + parent enemy 参照

```rust
struct EnemyOverheadHpBarRoot { enemy: Entity }   // 親 Enemy entity を保持
```

update system は overhead bar root から `EnemyOverheadHpBarRoot.enemy` を引き、その entity
の HitPoints を読んで ratio を計算する。Enemy が既に despawn 済みなら query.get が None で
skip (Bevy hierarchy で root も既に消えているので普通は起きない、保険的に処理)。

## Alternatives Considered

- **`enemy_hp_bar` に `world_anchored: bool` flag を生やす**: 同一 kind で screen / world を
  切替える案。world 用には anchor 系 field が全部無意味になり、struct の半分が「特定 mode で
  しか使われない」状態になる。`enemy_hp_bar` の YAML schema が膨らみ、editor UI も切替式の
  半分を隠す処理が必要。kind を分けるほうが clean。
- **`target` を `EnemyTarget` enum で受ける (`enemy_hp_bar` と統一)**: overhead で
  `last_engaged_by` は意味があり得る (engagement 中の enemy だけ overhead 表示) が、対象 enemy が
  1 体だけになり overhead の主用途 (全 enemy 上に表示) と乖離する。tag_filter 1 field のほうが
  YAML / UI 両方シンプル。`last_engaged_by` を overhead に欲しくなったら後で variant 追加可能。
- **per-frame spawn / despawn (Enemy 死ぬたびに bar も消す)**: Bevy hierarchy 自動破棄で
  既に正しく動くので追加コードは要らない。死亡演出中の HP 0 表示は update system が ratio=0
  で空 bar を描き、entity 消滅時に hierarchy 経由で消える。
- **world Y を Character の sprite height 自動計算で決める (旧案)**: 初版で「キャラ姿勢で
  sprite top が動くと bar も上下する不快感」を理由に固定値を選んだが、実際には頭上 bar が
  ジャンプで一緒に上に上がるほうが直感的だったため後付けで採用。`vertical_anchor: image_top`
  が標準形になった。固定値が欲しいケースには `vertical_anchor: origin` を残してある。

## Consequences

**得られたもの**

- 4 人 co-op で「画面上の各 enemy 上に小さい HP bar」が `enemy_overhead_hp_bar` 1 行で出せる。
- Boss 用に「太い赤枠の overhead bar」を tag_filter: boss で 1 設定で書ける。
- Enemy の spawn / despawn と完全同期 (Bevy hierarchy 任せ、明示 cleanup 不要)。
- Camera スクロール / camera follow と独立に Enemy 真上の固定位置に貼り付く (transform hierarchy)。
- ADR-0031 で導入した `Character.tag` と `EnemyTag` が tag_filter 用にそのまま再利用できた。

**支払うコスト**

- HUD kind が 4 つになり、`HudElement` accessor の match arm が 4 つ。新 variant 追加時の
  メンテナンス対象が増えた (ただし `is_screen_anchored` で分岐する形で各 accessor は実質
  既存の 3 variant + overhead の default 1 arm で固定形)。
- 後付けで `vertical_anchor: image_top/bottom` を入れたが、これらは **毎 frame
  Transform を再計算する**。画面上の overhead bar 数が多い (= 大量 enemy) と毎 frame の
  `sprite_pivot` / `image_dims` 読み出し + Transform 書き換えコストが乗るが、HUD 規模なら
  無視できる。`vertical_anchor: origin` は再計算が無いので静的バー用に残してある。
- `FrameRender` に `image_dims: [u32; 2]` を追加した (8 byte / frame)。AnimationFrames が
  メモリ上で多数の frame を持つキャラだと若干増えるが、誤差。
- spawn は `Added<Enemy>` で 1 度だけ走る前提だが、各 frame の Update に居る (1 frame 走査
  cost は new_enemies query が空ならゼロに近い)。OnEnter(Battle) ではなく Update で扱う
  のは Enemy が battle 中に `spawn_opponents_on_trigger` で逐次出現するため。

**今後の拡張余地**

- 「画像 sprite の外接 bbox を厳密に求める」 (= 透明 pixel を除いた tight bbox) を画像
  解析で取り、`vertical_anchor: visible_top` 等を追加する余地。現状は画像の枠 (含 transparent
  padding) を sprite top/bottom として扱う。
- overhead bar の damage 遅延 / 色変化を後付け (ADR-0029 の low_hp 系と同じ field を
  共通化)。
- player overhead bar (`player_overhead_hp_bar` kind) を同じ枠組みで追加可能。Player は
  PlayerId target になる以外は同じ形。

## 関連

- [ADR-0029](0029-hud-layout-in-project-yaml.md): HUD レイアウトを Project YAML に持つ。
  本 ADR も同じ internally-tagged enum の新 variant として接続。
- [ADR-0030](0030-multi-player-hud-targeting.md): PlayerId と HUD target。本 ADR の overhead は
  target を持たず tag_filter だけ。
- [ADR-0031](0031-enemy-hp-bar-and-engagement-tracking.md): screen-anchored enemy bar と
  Character.tag。本 ADR は tag を tag_filter で参照する。
- [ADR-0023](0023-image-pixel-world-screen-unification.md): world / bevy 座標変換。Enemy の
  Transform から bevy 局所 Y で持ち上げる挙動はこの規約に基づく。
- `packages/engine/src/features/hud.rs`: `spawn_enemy_overhead_hp_bars` / `update_enemy_overhead_hp_bar` /
  `EnemyOverheadHpBarRoot` / `EnemyOverheadHpBarGauge`
- `packages/engine/src/entities/project/model.rs`: `EnemyOverheadHpBarConfig` / `HudElement::is_screen_anchored`
- `packages/editor-desktop/src/features/hud/ui/edit_hud_layout.rs`: `EnemyOverheadHpBarEditor`
