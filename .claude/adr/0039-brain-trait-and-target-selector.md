# ADR-0039: Brain trait + TargetSelector enum で 3 Brain を宣言的に組む

## Status

Accepted (2026-06-28、Issue #7 で実装、MR !? でマージ予定)

## Context

ADR-0035 Phase 1-3 で 3 種類の AI Brain (`MeleeBrain` / `AllyBrain` / `BotBrain`) が揃い、
ADR-0038 (Side / Controller marker 分離) で Brain owner と target side のマッピングが
固定化された。本 ADR は ADR-0035 Phase 3 補追 §「先送り事項」で予告されていた:

> - Brain trait 抽出 (Melee / Ally / Bot の dedup)
> - `TargetSelector` enum (`Nearest` / `LastAttacker` / `Random`)

を実装する。動機は 2 点:

1. **3 Brain の高重複** ([memory: dedup_later] 維持中だった負債の清算)
   - FSM 遷移 (`decide_*_next_state` の Idle/Chase/Attack 部分) が 3 Brain で同型 (Melee と Bot は
     bit-identical、Ally は `Follow → Idle` の対応付けだけで同じロジックに乗る)
   - cooldown 管理 / decision_interval 間引き / dwell counter の boilerplate が 3 Brain で重複
   - 過去バグ「Attack 進入 frame で cooldown を仕込む順序を間違えて attack:true が次行で塗られる」
     ([ADR-0035] Phase 1.1 注釈) を 3 Brain で独立に守る運用が脆い
2. **target 選定戦略の初の実需** (Phase 1.1 で hardcode した Nearest の差し替え)
   - ADR-0035 Phase 2 補追で「Ally が最初に殴り始めた Villain を継続追跡したい」が浮上
   - Phase 1.1 §「Targeting は `EnemyAiBrain` 内に持つ」で「将来 `TargetSelector` enum
     (`Nearest` / `LastAttacker` / `Random`) で差し替え可能にする余地を残す」と decide 済み
   - ボス級 enemy で `WeightedByThreat` (ヘイト管理) が要る将来需要

ADR-0038 で attack resolve の disjoint を `MeleeBrain` の有無で取る規約が入った。本 ADR は
**Brain marker の type identity を壊さない** 範囲で dedup する (= 3 Brain 型は残し、共通基盤を
抽出する)。

## Decision

### 5 つの変更を同時に入れる

| 変更 | 対象 | 役割 |
|------|------|------|
| `EngagementConfig` 共通 sub-struct + `#[serde(flatten)]` | entities/character | Idle/Chase/Attack 周辺パラメータの dedup、YAML schema は flat field 維持 |
| `TargetSelector` enum (4 variant、2 stub) | entities/character | target 選定戦略の宣言的指定。`Nearest` / `LastEngaged` 実装、`Random` / `WeightedByThreat` は variant のみ |
| `EngagementState` 共通 enum | features/character/ai | MeleeBrain / BotBrain で同形だった `MeleeState` / `BotState` を統合。AllyBrain の `Follow` は Idle に等価マッピング |
| `BrainCounters` 共通 struct + helper (`tick_counters` / `apply_attack_cooldown` / `decide_engagement` / `engagement_command`) | features/character/ai | 3 Brain の per-frame ロジックを 1 箇所に集約、cooldown 仕込み順序の hidden invariant を helper に閉じる |
| `Brain` trait + `select_target` 自由関数 | features/character/ai | Brain ごとの aggregate アクセス契約。target 選定 dispatcher は QueryFilter generic な自由関数として切る |

### `EngagementConfig` を flatten で導入

`MeleeConfig` / `AllyConfig` / `BotConfig` は **chase_*/attack_*/cooldown/decision_interval/
dwell の 7 field が完全同形**だった。本 ADR では 7 field を `EngagementConfig` に切り出し、各
BrainConfig が `#[serde(flatten)] engagement: EngagementConfig` で展開する:

```rust
pub struct EngagementConfig {
    pub chase_enter_range_px: f32, pub chase_exit_range_px: f32,
    pub attack_enter_range_px: f32, pub attack_exit_range_px: f32,
    pub attack_cooldown_ticks: u32, pub decision_interval_ticks: u32,
    pub min_dwell_ticks: u32,
}

pub struct MeleeConfig {
    #[serde(flatten)] pub engagement: EngagementConfig,
    #[serde(default)] pub selector: TargetSelector,
}
pub struct AllyConfig {
    #[serde(flatten)] pub engagement: EngagementConfig,
    pub follow_distance_min_px: f32, pub follow_distance_max_px: f32,
    #[serde(default)] pub selector: TargetSelector,
}
pub struct BotConfig {
    #[serde(flatten)] pub engagement: EngagementConfig,
    #[serde(default)] pub selector: TargetSelector,
}
```

flatten 採用により **既存 YAML schema (`chase_enter_range_px: 200` 等のトップレベル flat
field) を編集なしで維持** できる。Rust 側のアクセスだけ `cfg.engagement.foo` に変わるが、これは
編集ルールどおりの機械的更新で済む。

### `TargetSelector` enum (4 variant、2 stub)

```rust
#[serde(rename_all = "snake_case")]
pub enum TargetSelector {
    #[default] Nearest,        // Phase 1.1 hardcode を enum 化
    LastEngaged,               // Phase 2 補追動機: Ally の継続追跡
    Random,                    // stub: warn + Nearest fallback
    WeightedByThreat,          // stub: ヘイト管理用、ボス級実装時に別 ADR で実装
}
```

YAML は `selector: nearest` / `selector: last_engaged` のような **plain string variant 形式**
(= snake_case 自動変換)。default = `Nearest` で既存 YAML は無編集で動作互換。

`Random` / `WeightedByThreat` は **variant 定義のみ + warn 1 回 (call site 単位) + Nearest
fallback**。これにより:

- YAML schema が future-safe (新 variant を増やしても breaking change にならない)
- 親 Issue #5 (BT / Utility AI トラッカー) の hate 管理実装が固まる前に enum を成長させられる
- 誤って `selector: random` を YAML に書いても挙動が破綻せず、warn ログでユーザーが気付く

### `EngagementState` + `BrainCounters` 共通基盤

```rust
pub enum EngagementState { Idle, Chase, Attack }

pub struct BrainCounters {
    pub frames_since_state_entered: u32,
    pub frames_until_next_decision: u32,
    pub attack_cooldown_remaining: u32,
}
```

3 Brain は struct field として `state: EngagementState` (MeleeBrain / BotBrain) または `state:
AllyState` (Ally — Follow 残し) + `counters: BrainCounters` を持つ。

`AllyState` (`Follow / Chase / Attack`) は Follow フェーズの semantic を保つため残し、engagement
判定時のみ `EngagementState` と相互変換する (`to_engagement` / `from_engagement` helper)。
`AllyState::Follow ↔ EngagementState::Idle` の対応付けで decide_engagement の Idle 分岐がそのまま
Follow → Chase/Attack 遷移として動く (= ロジック共有が成立する)。

### 3 helper で per-frame 重複を解消

```rust
fn tick_counters(counters: &mut BrainCounters, decision_interval_ticks: u32) -> bool;
fn decide_engagement(current: EngagementState, dist: f32, can_transition: bool,
                     eng: &EngagementConfig, cooldown: u32) -> EngagementState;
fn engagement_command(state: EngagementState, self_pos: &WorldPosition,
                      target_pos: &WorldPosition, eng: &EngagementConfig) -> AiCommand;
fn apply_attack_cooldown(counters: &mut BrainCounters, cmd: &mut AiCommand,
                         entering_attack: bool, attack_cooldown_ticks: u32);
```

`apply_attack_cooldown` に **「`*cmd = engagement_command(...)` の **後** に呼ぶ」hidden
invariant** を閉じ込めた。3 Brain がそれぞれ独立に守らなくて済む。

### `Brain` trait + `select_target` dispatcher

```rust
pub trait Brain {
    fn engagement(&self) -> &EngagementConfig;
    fn selector(&self) -> TargetSelector;
    fn counters(&self) -> &BrainCounters;
    fn counters_mut(&mut self) -> &mut BrainCounters;
    fn target(&self) -> Option<Entity>;
    fn set_target(&mut self, target: Option<Entity>);
}

fn select_target<M: QueryFilter>(
    self_pos: &WorldPosition,
    side_filter: Side,
    selector: TargetSelector,
    current_target: Option<Entity>,
    candidates: &Query<(Entity, &WorldPosition, &Side), M>,
) -> Option<(Entity, WorldPosition)>;
```

`Brain` trait は **アクセサ集合**として軽く使う (type contract)。`for_each_brain(world, ...)` の
ような world-level dispatch は組まない (= Bevy schedule の disjoint 解析と Query 型推論を壊さない
ため、`*_brain_tick` system は引き続き 3 本維持)。

`select_target` は trait method ではなく **QueryFilter generic な自由関数**。理由は Brain ごとに
target query の型 (`Query<...Without<MeleeBrain>>`) が異なり、trait method に generic を載せると
implementor 側の type erasure が複雑化するため。

`LastEngaged` の実装は **既存の `brain.target: Option<Entity>` を流用**:

- `current_target == Some(e)` かつ `candidates.get(e)` で取得可能 (= 生存)
- 取得した `&Side` が `side_filter` と一致 (= 同 side に遷移していない)

両方を満たすと前回 target を返す。それ以外は Nearest fallback。別 component
(`BrainTargetMemory(Option<Entity>)` 等) に切る案は YAGNI (本 ADR の scope では Brain 内 field で
十分、Mind Control 系で外部から強制したい需要が出たら別 component に切る)。

### debug overlay 拡張

F2 overlay の AI 行に `sel=<selector>` と `tgt=<entity_index>` を追加:

```text
AI=Chase  cd=0  dw=14  sel=Nearest      tgt=E12
AI=Attack cd=32 dw=4   sel=LastEngaged  tgt=E12
AI=Follow cd=0  dw=8   sel=Nearest      tgt=E03
```

- `sel` は `TargetSelector` variant を Debug format で表示
- `tgt` は `brain.target` の `Entity.index()`。`None` は `tgt=-`
- `format_ai_line` helper で 3 Brain 共通フォーマットに集約

## Alternatives Considered

- **`trait Brain { type State; type Config; fn decide(...); fn ai_command(...); }`** (Issue #7
  本文ドラフト): associated type 版 trait。dispatch を `for_each_brain(world)` 風に組めるが、
  Bevy Query / Component の型制約で generics が implementor (= MeleeBrain 等) に漏れる。
  Brain trait は accessor として軽く使い、ロジック共有は free helper で取る方が 3 Brain の
  Query 型差分を素直に表現できる。
- **`BotConfig` を新設せず `MeleeConfig` 流用 (ADR-0038 規約継続)**: Bot 専用 param が出る予兆が
  ない状態で型を分けるのは過早。ただし AiConfig::Bot に variant がある以上、Config 型が同形でも
  type identity (= YAML kind tag) は保つ価値があるため、本 ADR では `BotConfig` を `MeleeConfig`
  と同形の独立型として残した。
- **`EngagementState` を `MeleeBrain.state` 等の field 型として導入せず、helper にだけ使う**:
  Brain 型自身は `MeleeState` / `BotState` のまま、helper 入出力時に変換する案。dedup の効果が
  Brain 内に到達せず、struct 定義の同形重複が残るため不採用。
- **AllyBrain も `EngagementState` に統一 (Follow を Idle に統合)**: Follow と Idle で `move=0`
  挙動は同じだが、Follow は「Player に追従中」semantic を持つ (= debug overlay や将来の
  state-based 演出で差別化したい)。Follow を独立 state に残す方が情報損失なし。
- **`selector` を `AiConfig` enum 直下に持つ**: `AiConfig::Melee(MeleeConfig) + selector` のように
  variant 横断 field を生やす案。`#[serde(tag = "kind")]` の internally-tagged enum で variant
  外 field を持たせると外部表現が二重 tag になり editor UI / シリアライザを混乱させる。
  Per-Config field の方が素直。
- **`TargetSelector` を `#[serde(tag = "kind")]` の internally-tagged enum で導入**: 将来 variant
  が field を持つ (`Random { seed: u32 }` 等) ことを見越した案。本 ADR では全 variant が
  fieldless なので plain string で十分。fieldful variant が必要になったタイミングで migration
  すれば良い (1 字 variant 名で対応可能で breaking ではない)。
- **`Random` / `WeightedByThreat` を本 ADR で実装する**: scope が膨らみ、`WeightedByThreat` は
  hate 計算の仕様 (= 別 ADR) が必要。stub + warn fallback で variant スキーマだけ固めて、実装は
  実需が出てから別 Issue で行う方が変更幅を絞れる。
- **`LastEngaged` 用 state を別 component `BrainTargetMemory` に切る**: Brain 外から target を
  強制する use case (mind control / scripted demo) を想定した案。本 ADR の scope ではそれらの
  use case はなく、`brain.target` の流用で十分。

## Consequences

**得られたもの**
- `MeleeBrain` / `BotBrain` / `AllyBrain` の per-frame ロジックが 4 helper
  (`tick_counters` / `decide_engagement` / `engagement_command` / `apply_attack_cooldown`) に
  集約。新 Brain 型を追加する際の追従コストが下がる
- cooldown 仕込み順序の hidden invariant が `apply_attack_cooldown` 1 箇所に閉じた
  (Phase 1.1 の独立守護が不要)
- `TargetSelector::LastEngaged` で Ally の継続追跡が宣言的に YAML から指定可能になり、
  ADR-0035 Phase 2 補追の実需に応えた
- `Random` / `WeightedByThreat` を variant 定義だけ確保したことで、将来の hate 管理
  実装で YAML schema を再変更せずに済む base ができた
- debug overlay の `sel` / `tgt` 行で「現 frame の selector + target」が目で追えるようになり、
  検証コストが下がった

**支払うコスト**
- 既存 Brain の Rust 側 field アクセスが `brain.config.engagement.chase_enter_range_px` /
  `brain.counters.attack_cooldown_remaining` 等の **二段深い** path に変わった。
  読みやすさは「flat field 直アクセス」と比べて落ちるが、`EngagementConfig` の semantic 集約
  を取った
- `MeleeState` / `BotState` を削除し `EngagementState` に統合したことで、外部 (editor 等) からの
  参照は壊れる。現状本 repo 内では engine 内のみ参照しており影響なし (確認済)
- `Brain` trait は accessor 集合に留めたため「Brain の dispatch を統一する」直接的なメリットは
  少ない。trait は将来 Brain ごとの uniform 操作 (`brain.set_paused(true)` 等) が必要になった
  時に拡張するための base structure としての価値が主

**今後の拡張余地**
- `TargetSelector::Random` / `WeightedByThreat` の **挙動実装** は実需 (Random demo /
  ボス級 enemy hate 管理) が出た時点で別 Issue。`WeightedByThreat` は scoring 関数の仕様
  (距離 / HP / 最終被弾時刻 / engagement_link 等) を別 ADR で起こす
- editor UI の `selector` dropdown (Issue #8) — 本 ADR で enum が固まったので追加可能
- Brain trait 経由の world-level dispatch (`for_each_brain(world, |brain| brain.tick(...))`) は
  schedule disjoint と Query 型推論の整合がついたら導入できる base ができた
- `EngagementConfig` の per-frame 動的変更 (= 戦闘中の reaction 速度調律) — 例えば
  「low HP で `decision_interval_ticks` を縮めて反応速度を上げる」のような hot tuning は
  本 ADR の `BrainCounters` + `EngagementConfig` 分離により Brain trait method 1 つ追加で
  実現可能

## 関連

- [ADR-0035](0035-character-ai-three-layer-fsm.md): Brain / Intent / Actuator 3 層分割の origin。
  Phase 1.1 §「Targeting は EnemyAiBrain 内に持つ」での `TargetSelector` 予告、Phase 2 補追
  §「`AllyConfig` パラメータ」での「3 つ目の Brain が出てきたら共通項を見て判断」、Phase 3 補追
  §「先送り事項」での Brain trait 抽出 + TargetSelector enum 化の予告を実装
- [ADR-0038](0038-side-controller-marker-separation.md): Brain owner と target side のマッピング
  固定 (`MeleeBrain` = Villain attacker / Hero target、`BotBrain` = Hero attacker / Villain
  target、`AllyBrain` = Hero NPC / Villain target with Hero+Human follow fallback)。本 ADR の
  `select_target(side_filter, ...)` 引数として直接利用
- [ADR-0031](0031-enemy-hp-bar-and-engagement-tracking.md): `LastEngagedWith` (Player → Enemy)。
  本 ADR の `TargetSelector::LastEngaged` は別概念 (Brain 内 `brain.target` 継続)。HUD の
  `engagement-link` 起点とは独立で、将来 `TargetSelector::LastEngagedHuman` のような cross-cut
  variant が必要なら別 ADR で
- Issue #7 — 本 ADR の実装 Issue
- Issue #5 (umbrella、open) — BT / Utility AI トラッカー。本 ADR の `TargetSelector` stub
  variant が成長段階で本 Issue から fork される想定
- Issue #8 — editor UI 対応 (`selector` dropdown / debug overlay の color coding)
- `packages/engine/src/entities/character/model.rs`: `EngagementConfig` / `TargetSelector` /
  3 BrainConfig の flatten 再編
- `packages/engine/src/features/character/ai.rs`: `EngagementState` / `BrainCounters` / `Brain`
  trait / 4 helper / `select_target` / 3 Brain の再実装
- `packages/engine/src/features/character/state_debug.rs`: F2 overlay の `sel` / `tgt` 行追加
- `packages/engine/src/scenes/battle.rs`: spawn 経路 (selector は Config 経由で透過的に流れるため
  追加変更なし)
