# ADR-0033: Player Icon HUD と独自 shake trigger

## Status

Accepted (2026-06-25 に `player_icon` kind と独立 shake trigger を追加)

## Context

ADR-0029 で導入した `HudElement` enum には HP 系の要素 (bar / ring / overhead) は揃ったが、
**Player の現状態を象徴するアイコン** (= 顔絵 / 立ち絵カットイン的なもの) は無かった。
beat-em-up の HUD では「ピンチで顔色が変わる」「攻撃を当てた瞬間に表情が変化する」「被弾で
アイコンが揺れる」といった視覚的フィードバックが一般的で、これを表現する場所が必要だった。

要件:

1. **State ごとに Icon を切替える**: Idle / Walk / Attack / Hit / ... の各 `CharacterState`
   ごとに別画像を出せる。未指定 state は default 画像にフォールバック。
2. **HitPause みたいに振動するエフェクト**: 「被弾したとき」「攻撃を当てたとき」など特定の
   trigger で Icon HUD だけが揺れる。Player 本体の HitStop 振動とは独立した制御が欲しい
   (= Icon 専用に強い振動を当てる、または HitStop 中も Icon は振らない、を作品ごとに選べる)。

なお、`CharacterState` enum は `features/character/state_machine.rs` にあり、FSD 規約上
`entities/project/model.rs` (= entities layer) からは参照できない。state を map key に
取るには別の表現が必要。

## Decision

### `HudElement::PlayerIcon` を新規 kind として追加

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HudElement {
    PlayerHpBar(PlayerHpBarConfig),
    PlayerHpRing(PlayerHpRingConfig),
    EnemyHpBar(EnemyHpBarConfig),
    EnemyOverheadHpBar(EnemyOverheadHpBarConfig),
    PlayerIcon(PlayerIconConfig),   // 新規
}

pub struct PlayerIconConfig {
    pub id: Option<String>,
    pub target: PlayerId,
    pub anchor: HudAnchor,
    pub anchor_to: Option<HudElementAnchor>,
    pub offset: HudOffset,
    pub size: HudSize,
    pub frame: HudFrame,
    pub bg_color: HexColor,
    pub sprite_group_number: u32,            // character の sprite_groups[N] を指す
    pub default_sprite_index: u32,           // state_sprites に無い state のフォールバック
    pub state_sprites: HashMap<Role, u32>,   // Role -> sprite_index
    pub shake: IconShakeConfig,              // 振動 trigger / params
}
```

```yaml
hud:
  elements:
    - kind: player_icon
      target: p1
      anchor: top_left
      offset: { x: 8, y: 40 }
      size: { w: 40, h: 40 }
      frame: { thickness: 1, color: "#000" }
      sprite_group_number: 100        # hero/sprite-groups/icons.yml (number: 100)
      default_sprite_index: 0
      state_sprites:
        idle: 0
        walk: 0
        attack: 1
        hit: 2
        knockback_up: 2
      shake:
        on_damage: { duration_ms: 200, shake_x: 0, shake_y: 3, count: 4, decay: 1.0 }
        on_attack_hit: { duration_ms: 80, shake_x: 2, shake_y: 0, count: 2, decay: 0.5 }
```

### state map の key に `Role` (entities/character) を採用

`CharacterState` (`features/character/state_machine.rs`) は FSD 上、entities layer から
参照できない。一方 `Role` (`entities/character/model.rs`) は既に
`#[derive(Hash, Eq, Serialize, Deserialize)]` が入っており HashMap key にできる。
state machine 側で **既に存在する** `CharacterState::to_role()` を経由して引けば、
project YAML は `idle: ...` / `attack: ...` / `knockback_up: ...` の snake_case Role 名で
state map を書ける。Custom variant も含めて serde で素直に deserialize できる。

### Character の sprite_groups を専用 group として流用

「Icon 専用の sprite_group を 1 つ用意する」運用にする (例: `hero/sprite-groups/icons.yml`
で `number: 100`、`sprites: [{ index: 0, ... }, { index: 1, ... }, ... ]`)。
HUD config の `sprite_group_number` がその group を指し、`state_sprites` で各 Role に
対応する `sprite_index` を割り当てる。

別案として「Role ごとに `(group_number, sprite_index)` を両方指定する」設計も検討したが、
- 1 つの group に icon 用の sprite を集めるほうが asset 管理上で見通しが良い
- HUD config の YAML 行数が短くなる
- editor 側で sprite picker を作りやすい (1 group の中から sprite_index を選ぶ UI で済む)

ので、**1 config = 1 group** に絞った。複数 group を跨ぐ需要が出てきたら variant を増やす。

### `PlayerSpriteGroupRegistry` resource で sprite_groups を battle 中持ち越し

`battle::setup` 内で `Character::load_directory` の戻り値は library 構築後に drop されて
いた。HUD spawn から sprite_groups を引くため、`PlayerSpriteGroupRegistry` resource を新設し、
`PlayerId -> { character_name, sprite_groups }` を保持する。

```rust
#[derive(Resource, Default)]
pub struct PlayerSpriteGroupRegistry { by_player: HashMap<PlayerId, PlayerSpriteGroups> }
pub struct PlayerSpriteGroups {
    pub character_name: String,
    pub sprite_groups: HashMap<u32, SpriteGroup>,
}
```

`PlayerAnimationLibrary` (Role -> AnimationData) と並列に置く。前者は描画用の cache、
後者は HUD が自由に sprite を引くための raw 保管庫。

### Shake は HitStopState 連動ではなく独自 trigger

設計選択肢として「HitStopState (= Player 本体に attach される hit-stop 振動) を Icon に伝搬」
が最も実装が軽かったが、要件として:

- **on_damage** (HP が減った瞬間): 被弾でも HitStop が発生しないケース (ガード damage で
  guard_gauge だけ削れる等) でも Icon は揺らしたい
- **on_attack_hit** (Player が attack を当てた瞬間): 攻撃側 hit-stop は visual shake 無し
  (`HitStopState::attacker(...)` で shake パラメータゼロ) なので、Icon を意図的に揺らすには
  別の trigger 経路が必要

があり、HitStopState 連動だと表現力が足りない。**Icon HUD 専用の独自 trigger を 2 種類**
持たせ、それぞれ独立した `IconShakeParams` (HitStop と同じ三角波 + 線形減衰 schema) を
当てられる設計にした:

```rust
pub struct IconShakeConfig {
    pub on_damage: Option<IconShakeParams>,
    pub on_attack_hit: Option<IconShakeParams>,
}
pub struct IconShakeParams {
    pub duration_ms: u32,
    pub shake_x: i32,    // 画面 X (+ = 右)。HitStop と違い Facing 反転しない
    pub shake_y: i32,    // 画面 Y (+ = 上)
    pub count: u32,      // 三角波の片道回数。0 で振動なし
    pub decay: f32,      // 線形減衰、1.0 で末尾の振幅 0
}
```

trigger 検出:

- **on_damage**: `PlayerIconRoot` に `last_hp` を持たせ、毎 frame `HitPoints.current` と
  比較。`current < last_hp` で発火 → `last_hp = current` で更新。
- **on_attack_hit**: `Added<HitStopState> + With<Player>` で attacker 側 attach を検出
  (attack.rs の `resolve_hits` が `HitStopState::attacker(...)` を insert する)。

振動の適用は `IconShakeState` component を root に attach し、`tick_icon_shake` system が
HitStop と同じ計算式で Transform.translation に offset を載せる。`base_translation` は
spawn 時の root 位置を覚えておき、毎 frame `base + offset` で上書き → 残時間が 0 で
component を remove + base に戻す。

`icon_triangle_wave` 関数は HitStop の `triangle_wave` と同じ計算だが、敢えて DRY 化せず
各 slice 内に複製した (CLAUDE.md「先に共通基盤を作らない」、ADR-0001 の FSD slice 独立性)。
将来 3 個目の使い手が出てきたら `shared::math` に押し上げる。

## Alternatives Considered

- **HitStopState を Icon にも伝搬**: 実装が最小 (Player に attach された HitStopState を
  そのまま読むだけ) だが、被弾 ≠ HitStop (ガード時に hit_stop なし) のケースで Icon を
  振れない。表現力不足で却下。
- **`HudElement::PlayerIcon` ではなく `PlayerHpBar.icon: Option<...>` で組み込み**: bar の
  サブ要素にする案。位置決め (anchor_to + offset) が bar に縛られ独立配置できないのと、
  「bar 無しで Icon だけ」のケースが書けない。kind を分ける方が直交。
- **state map に `CharacterState` を直接使う**: FSD 上、entities/project から features を
  参照することになりレイヤ規約違反。`Role` を介する迂回が clean。
- **state -> (group_number, sprite_index) の両方指定**: 柔軟だが冗長。1 group / 多 sprite が
  asset 管理の自然形なので採用見送り (Decision の通り)。
- **Icon 専用の sprite asset を `data/hud-icons/...` に置く新ディレクトリ**: character data と
  完全に切り離せる案。だが Player Icon は **Player キャラに紐付いた表現** なので、
  character 配下 (sprite_groups) に置く方が「キャラを差し替えたら icon も差し替わる」の挙動が
  自然に成立する。

## Consequences

**得られたもの**

- Player の現状態に応じた Icon HUD が 1 kind で表現可能 (state -> sprite_index map で完結)。
- 被弾 / 攻撃当てを trigger にした演出を Icon HUD だけに当てられる (Player 本体は振らさず
  Icon だけ強く揺らす、等のチューニングが project YAML だけでできる)。
- 既存の Character.sprite_groups エコシステムを再利用。editor 側で「Icon 用 group」を
  普通に sprite_group_editor で編集できる (新 UI は不要)。
- `PlayerSpriteGroupRegistry` resource は今後別の HUD 要素 (将来の `player_overhead_*` や
  カットイン演出) からも sprite_groups を引ける汎用 channel として再利用可能。

**支払うコスト**

- HUD kind が 5 つに増え、`HudElement` accessor の match arm が 5 つ。新 variant 追加時の
  メンテ対象が 1 つ増えた (既存パターンを踏襲する形ではあるので増加分は機械的)。
- `Character.sprite_groups` を `PlayerSpriteGroupRegistry` に **clone** して持ち越す
  (= setup 1 回のみのコスト)。`HashMap<u32, SpriteGroup>` は深くないので memory 増加は誤差。
- shake の 三角波計算が 2 箇所 (hit_stop / hud) で重複。3 個目が出たら shared に集約予定。
- editor 側の UI 編集 (HUD 要素エディタ) は本 ADR の範囲外で別途必要。現状 project.yml の
  直接編集でしか player_icon を書けない。

**今後の拡張余地**

- shake の trigger に `on_state_change` や `on_state_enter(Role)` を追加可能 (= 特定 state に
  入ったとき限定の演出)。`IconShakeConfig` への field 追加で済む。
- `state_sprites` の値を `sprite_index` だけでなく flip 指定 (mirror icon) や tint 色まで
  拡張する余地。`HashMap<Role, IconAppearance>` に昇格すれば段階的に拡張可能。
- 複数 group を跨ぐ需要が出てきたら `state_sprites: HashMap<Role, IconRef { group: u32, index: u32 }>`
  に拡張可能 (1 group 縛りは current の選好に過ぎず、後付け可能)。
- Player キャラ複数体 (`PlayerId::P2` / P3 / P4) の icon を独立指定可能
  (`PlayerSpriteGroupRegistry` は player_id keyed なので、Phase 1 の MVP は P1 のみだが
  multi-player Phase で拡張は機械的)。
- editor 側に PlayerIcon の WYSIWYG エディタを追加 (state_sprites の grid UI、shake preview)。

## 関連

- [ADR-0029](0029-hud-layout-in-project-yaml.md): HUD レイアウトを Project YAML に持つ。
  本 ADR も同じ internally-tagged enum の新 variant として接続。
- [ADR-0030](0030-multi-player-hud-targeting.md): PlayerId と HUD target。本 ADR の player_icon は
  `target: PlayerId` で対象 Player を指す。
- [ADR-0031](0031-enemy-hp-bar-and-engagement-tracking.md): screen-anchored 系の anchor_to / id。
  本 ADR も同じ枠組みで他要素から参照可能。
- [ADR-0001](0001-adopt-feature-sliced-design.md): FSD slice 独立性。`icon_triangle_wave` を
  共通化しない判断の根拠。
- `packages/engine/src/features/hud.rs`: `spawn_player_icon` / `update_player_icon_sprite` /
  `detect_icon_damage` / `detect_icon_attack_hit` / `tick_icon_shake` / `PlayerIconRoot` /
  `PlayerIconSprite` / `IconShakeState` / `icon_triangle_wave`
- `packages/engine/src/entities/project/model.rs`: `PlayerIconConfig` / `IconShakeConfig` /
  `IconShakeParams` / `HudElement::PlayerIcon`
- `packages/engine/src/features/character/state_machine.rs`: `PlayerSpriteGroupRegistry` /
  `PlayerSpriteGroups`
- `packages/engine/src/scenes/battle.rs`: `PlayerSpriteGroupRegistry::insert` 経路
