# ADR-0035: キャラクター AI を Brain / Intent / Actuator の 3 層で導入する

## Status

Proposed (2026-06-27 ドラフト、Issue #1 で議論中)

## Context

Beat-em-up runtime に **キャラクターの AI** を入れたい。現状:

- `packages/engine/src/features/character/ai.rs` は雛形 (1 行 doc のみ) で、実装はゼロ。
- `CharacterState` (state_machine.rs:30) は Player / Enemy で共用、`is_locked()` な state は
  「入力 / AI による上書きを受け付けない」と doc にある。AI レイヤが state を駆動する設計余地が
  残されている。
- `handle_input` (movement.rs:110) は `ButtonInput<KeyCode>` を直接読んで `CharacterState` /
  `KinematicVel` / `Facing` に書き込む。生入力と state 駆動が同じ system に同居している。
- Enemy entity は `Enemy` marker + `EnemyTag` (ADR-0031) を持つが、行動を決めるコードが
  どこにも無い。床に立って Player に殴られるだけの的。
- Player 個体識別 (`Player(PlayerId)`, ADR-0030) と Player→Enemy の engagement
  (`LastEngagedWith`, ADR-0031) は揃っている。逆方向 (Enemy→Player の追跡 target) は未実装。

AI 対象として 3 種類想定:

1. **Enemy NPC** — 雑魚 / 中ボス / ボス。beat-em-up の主戦場。
2. **味方 / 仲間 NPC** — Player と共闘する相棒。
3. **Player 自動化** — デモプレイ / オートバトル / デバッグ bot。

これらを別々に実装すると Player の入力解決ロジック (`is_locked` / Guard / Jump の優先度、Attack
発火条件など) が 3 箇所に散る。一方で「Enemy AI も Player と同じ Actuator を通す」形にすれば、
state 駆動の優先度判定は 1 箇所に集約できる。

## Decision

### 3 層 (Brain / Intent / Actuator) に分割する

```
[Brain]    *AiBrain (FSM)        per-character、target / state を持ち毎 tick で意思決定
             ↓ writes
[Intent]   AiCommand component   Player の生入力と等価な「意図」表現 (per-entity)
             ↓ read by
[Actuator] apply_command system  AiCommand → CharacterState / KinematicVel / Facing
                                  is_locked / Guard / Jump の優先度判定はここに集約
```

3 つのスコープは Brain だけ違う:

| スコープ | Brain | Actuator |
|---|---|---|
| Enemy NPC | `EnemyAiBrain` (FSM) | `apply_command` (共通) |
| 味方 NPC | `AllyAiBrain` (FSM) | `apply_command` (共通) |
| Player 自動化 | `BotBrain` (scripted / recorded) | `apply_command` (共通) |
| 通常 Player | `PlayerInputController` (`ButtonInput` → `AiCommand`) | `apply_command` (共通) |

**現 `handle_input` は 2 段に割る**: `PlayerInputController` (生入力 → AiCommand) と
`apply_command` (AiCommand → state)。これにより Player と AI controlled entity が
対称な構造になり、Actuator は 1 つだけ。

### `AiCommand` のスキーマ (desire ベース)

```rust
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AiCommand {
    /// 移動入力。-1.0..=1.0 で各軸独立、現 handle_input の dx/dz と等価。
    /// 大きさは Actuator 側で MOVE_SPEED_PX_PER_SEC * dt に乗じる。
    pub move_x: f32,
    pub move_z: f32,
    /// 殴りたい間 true (= desire)。`apply_command` 側で「state が Idle/Walk のときだけ
    /// Attack 遷移を許可」することで just_pressed 相当の edge を暗黙生成する。AI Brain は
    /// 攻撃したい間ずっと true を出すだけでよく、edge 管理を自前でやらない。
    pub attack: bool,
    pub down_attack: bool,
    pub jump: bool,
    /// pressed 相当 (押下中ずっと true)。Guard の継続保持に必要。
    pub guard: bool,
    /// 強制向き。None なら move_x で更新 (現 handle_input と同じ規約)。
    pub face: Option<Facing>,
}
```

**`attack` / `down_attack` / `jump` を desire (= 押下継続) で表現する設計**:

- Player 側は `attack: action_map.pressed(...) && !state.is_locked()` のような単純変換で済む
  (= 現 `just_pressed` ベースの edge 化を捨てて Actuator 任せにする)。
- AI 側は「Attack したい状況になったら `attack: true` を出し続け、Idle に戻った瞬間に
  `apply_command` が 1 回だけ `CharacterState::Attack` に遷移させる」流れで、edge 管理を
  Brain が自前でやらなくて済む。`MeleeBrain` の attack_cooldown も「cooldown 中は
  `attack: false` を返す」と Brain 内ロジックだけで完結。
- Guard は元から pressed (押下中継続) 規約なのでそのまま (現 `handle_input` も `pressed`)。

### Phase 1: Enemy 用 `MeleeBrain` (FSM 3 状態 + チャタリング防止)

```rust
#[derive(Component, Debug, Clone)]
pub struct MeleeBrain {
    pub state: MeleeState,
    pub target: Option<Entity>,
    pub state_entered_at_tick: u64,    // dwell time 判定用 (現 tick との差で滞在 tick を測る)
    pub frames_until_next_decision: u32,
    pub attack_cooldown_remaining: u32,
    pub config: MeleeConfig,
}

pub enum MeleeState { Idle, Chase, Attack }

pub struct MeleeConfig {
    /// Chase に入る距離 (target がこの距離内なら Chase)
    pub chase_enter_range_px: f32,
    /// Chase から Idle に戻る距離 (chase_enter < chase_exit で hysteresis)
    pub chase_exit_range_px: f32,
    /// Attack 発火距離 (この距離内なら Attack)
    pub attack_enter_range_px: f32,
    /// Attack をやめて Chase に戻る距離 (attack_enter < attack_exit で hysteresis)
    pub attack_exit_range_px: f32,
    /// 攻撃後の cooldown
    pub attack_cooldown_ticks: u32,
    /// N frame ごとに reschedule (反応速度の調律点)
    pub decision_interval_ticks: u32,
    /// state 遷移後の最低滞在 tick 数 (これ未満で次遷移は抑制)
    pub min_dwell_ticks: u32,
}
```

遷移は単純: Idle → (target 検出 + chase_enter 距離内) → Chase → (attack_enter 距離内) →
Attack → (locked 終了) → Idle。Brain 自身は state 遷移と AiCommand 書き込みだけ担い、
`CharacterState` には直接触らない。

**Brain は component 単体 attach (enum でラップしない)**: Phase 1 は `MeleeBrain` 一種しか
無いので、`EnemyAiBrain` のような enum でラップする必要は無い (YAGNI)。将来 `RangedBrain` 等
2 種類目が出てきたら、enum 化と Brain ごとの tick system 分離のどちらが筋良いかを再評価する。

### チャタリング防止 3 軸 (Brain + Actuator)

AI が境界付近で振動するのは他エンジン経験で頻発する罠 (Walk ⇄ Idle の小刻み切替、Facing の
左右ピクピク、Attack のオン/オフ点滅)。原因は「瞬間 + 閾値 1 本」の判定。3 軸で構造的に防ぐ:

1. **ヒステリシス (2 段閾値)** — `MeleeConfig` の range を `_enter_` / `_exit_` の 2 本ずつ
   持つ。例: `attack_enter=28, attack_exit=36` で 8 px の dead zone を作り、境界振動を吸収。
   ヒステリシス幅 (= exit - enter) は character の歩行速度 (80 px/sec @ 60Hz = 約 1.3 px/frame)
   を踏まえ、`decision_interval_ticks` 1 周期で進む距離より十分大きく取る。
2. **State dwell time (Brain)** — `MeleeBrain.state_entered_at_tick` を遷移時に記録し、
   `current_tick - state_entered_at_tick >= min_dwell_ticks` を遷移の必須条件にする。
   例: `min_dwell_ticks = 8` (= 約 130ms) で「1 frame で Walk → Idle → Walk」を構造的に不能化。
3. **Facing dead zone (Actuator)** — `apply_command` で `cmd.move_x.abs() < FACING_DEAD_ZONE`
   (例 0.15) のとき `Facing` を更新しない。AI 側で move_x が微小に振動しても向きは固定される。
   Player 側は `PlayerInputController` が move_x を ±1.0 or 0 のデジタル入力で出すので影響
   ゼロ、AI 側だけが恩恵を受ける。現 `handle_input` の「dx == 0 なら facing 維持」規約は
   このルールの degenerate case として包含される。

既存の `decision_interval_ticks` (Brain 自体の間引き) も振動周期を伸ばす方向に寄与するが、
これは「振動を遅くする」だけで根絶ではない。**主軸は 1 + 2 で構造的に防ぎ、3 で見た目を守る**。

### Targeting は `EnemyAiBrain` 内に持つ

`MeleeBrain.target: Option<Entity>` で持ち、Brain tick の冒頭で再評価する。default の
選定戦略は **「最も近い Player」**。マルチ Player (ADR-0030) の 4 人 co-op を想定し、
将来 `TargetSelector` enum (`Nearest` / `LastAttacker` / `Random`) で差し替え可能にする
余地を残す。enum は Phase 2 で実需が出てから導入し、Phase 1 は Nearest hardcode。

### YAML スキーマ: `Character.ai` field

```yaml
# sample-projects/minimal/characters/grunt/character.yml
name: grunt
tag: null
ai:
  kind: melee
  chase_enter_range_px: 200.0     # 視認 → Chase
  chase_exit_range_px:  240.0     # ロスト → Idle (40 px hysteresis)
  attack_enter_range_px: 28.0     # 攻撃発動
  attack_exit_range_px:  36.0     # 離脱 → Chase (8 px hysteresis)
  attack_cooldown_ticks: 60
  decision_interval_ticks: 6
  min_dwell_ticks: 8              # state 遷移後の最低滞在 tick
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiConfig {
    Melee(MeleeConfig),
}

pub struct Character {
    // ...既存 field...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiConfig>,
}
```

internally-tagged enum で 1 kind = 1 variant。`ai: null` (= None) なら従来通り「何もしない
オブジェクト」として扱う。Player キャラには ai を書かなければ良いだけ。

**ai/<name>.yml のような別ファイル分離は採用しない** (Phase 1)。AI 定義はキャラと 1:1 で
始め、複数キャラで AI を共有する需要が出てから別 ADR で扱う。

### AI tick: 60Hz Update + Brain 内で間引き

Brain system 自体は Update に乗せ (ADR-0019 / 0027 と同じ `SimulationSet::Active`)、
`frames_until_next_decision` で per-brain に間引く。雑魚は 6 frame (100ms) ごと、ボスは
3 frame ごと、のように character YAML で調律する。

system 自体の Schedule rate を変えると hit_stop / pause / debug_control の連動が複雑化
するため、tick rate は固定して間引きは Brain ローカル変数で吸収する。

### system 順序

```text
SimulationSet::Active 内:
  PlayerInputController.read_inputs    # ButtonInput → 自 entity の AiCommand
  EnemyAiBrain.tick                     # FSM + world state → 自 entity の AiCommand
  apply_command                         # AiCommand → CharacterState / KinematicVel / Facing
  (以降は既存の sync_transform / sync_flip / camera_follow / AttackSet::Resolve など)
```

`apply_command` は現 `handle_input` のロジック (Guard > Jump > Attack > DownAttack > 移動
の優先度、`is_locked()` skip、空中 / 地上判定) をそのまま引き継ぐ。差分は「ButtonInput を読む
代わりに `&AiCommand` を読む」だけ。

### Debug 可視化

`features/character/state_debug.rs` の overlay に 1 行追加:

```text
P1  Idle    pos (120, 0, 80)  facing Right
E0  Chase   target P1         cooldown 0    [Melee]
E1  Attack  target P1         cooldown 32   [Melee]
```

`F3` toggle で表示 (現 state_debug の hot-toggle 機構をそのまま使う)。

### Phase 段階

| Phase | 内容 |
|---|---|
| 1.0 | Player の `handle_input` を `PlayerInputController` (ButtonInput → AiCommand) + `apply_command` (AiCommand → state) に分割。**挙動は変えない**、既存の手動操作シナリオで回帰確認できることがゴール |
| 1.1 | `MeleeBrain` (Idle/Chase/Attack) を追加し、sample-projects/minimal の grunt に attach。Enemy が初めて殴り返してくる |
| 2 | `AllyBrain` (味方 NPC)、Player follow / 共闘ロジック。Enemy ↔ Ally のすり替え (シナリオ加入離脱) もここで扱う (実装入り: [Phase 2 補追](#phase-2-補追-2026-06-27) 参照) |
| 3 | `BotBrain` (Player auto)、デモプレイ / replay。Player auto 化のすり替えはここで実装 |
| 4+ | `TargetSelector` enum、ボス用 BT / Utility 検討、editor からの AI param 編集、**Side / Controller marker 分離** (= Mind Control 系の本格すり替え対応、下記 [Consequences §今後の拡張余地] 参照) |

## Alternatives Considered

- **中間表現を挟まず、Brain が直接 `CharacterState` を書く**: Phase 1 は最小コストだが、
  Guard / Jump / Attack の優先度判定と `is_locked()` skip を Player 側 `handle_input` と
  Enemy Brain で **二重に書く** ことになる。優先度ルールが変わるたびに両方を直す保守コストが
  高い。中間表現を挟むと apply_command 1 箇所で済む。
- **Brain trait を最初から切る** (`trait Brain { fn tick(&mut self, ...) -> AiCommand }`):
  「共通項が見えないまま設計を固める」罠 (memory: dedup_later)。Phase 1 は `EnemyAiBrain`
  enum の具体型で書き、味方 / Player auto を実装する Phase 2 / 3 で trait 化を再評価。
- **BT / Utility AI を最初から導入**: 雑魚 1 体動かすのに過剰。FSM で書いてみて状態爆発したら
  BT に上げる「段階移行」を取る。`AiCommand` を挟んでいるので Brain 実装の差し替えは Actuator
  に影響しない。
- **AI 定義を `ai/<name>.yml` で別ファイル**: 「同じ AI を複数キャラで共有」需要が出てから。
  Phase 1 は character YAML 内で完結し、ADR-0011 (YAML primary) の 1:1 関係を維持する。
- **AI tick を独立 Schedule (例: 10Hz FixedUpdate) で回す**: 反応速度の調律はやりやすいが、
  hit_stop / pause / debug_control との同期が崩れる (movement.rs:86-91 で同じ理由で
  FixedUpdate を避けている)。tick rate を固定して Brain 内変数で間引く方が安全。
- **Targeting を独立 component `Targeting` で持つ**: Brain 外から target を参照したい需要
  (例: HUD で「敵の追跡対象を矢印で表示」) が出てきたら独立 component に切り出す。Phase 1 は
  Brain 内の field で十分。
- **Player の `handle_input` を割らず、現状維持で Enemy だけ Brain → AiCommand →
  apply_command_for_ai_only**: Actuator が 2 つに分かれる。Player / AI で `is_locked()`
  skip の扱いがズレる事故の余地が増える。Player を割って合流させる方が長期保守は楽。

## Consequences

**得られたもの**

- Enemy / 味方 / Player auto を「同じ Actuator + 違う Brain」で実装できる。優先度ロジックの
  二重管理が消える。
- FSM → BT / Utility への移行が Brain 差し替えで済む (Actuator / Intent 層は無傷)。
- AI tick の間引きが Brain ローカル変数で済み、system 順序を変えずに反応速度を調律できる。
- character YAML 1 ファイルで AI を含むキャラ定義が完結する (ADR-0011 と整合)。

**支払うコスト**

- 現 `handle_input` の Player 入力経路を 2 段に割るリファクタが入る。既存の挙動は維持する
  必要があり、Phase 1 で先に「ButtonInput → AiCommand → apply_command」が Player だけで
  動くことを確認してから Enemy を載せる順序が要る。
- `AiCommand` component が全 character entity (Player + Enemy) に attach される。空き
  component を持つコストは Bevy では無視できるが、bundle tuple が 1 個増える。
- Brain enum / FSM の追加で `features/character/ai.rs` が雛形から実装ファイルに変わる。
  FSD 規約 (engine の features は segment 無し) に従い slice 直下にファイル直書き。

**今後の拡張余地 (Phase 2+)**

- `AllyBrain` は「Player を追いかけて近くの Enemy を殴る」FSM として `MeleeBrain` の
  ロジックの 80% を共有できる。共有部の抽出は実装してから判断。
- `BotBrain` は「録音した AiCommand を時系列で吐く replay brain」と「scripted demo brain」を
  enum 切替で持てる。
- `TargetSelector` enum で `Nearest` / `LastAttacker(LastEngagedWith 逆引き)` / `Random` を
  切り替え可能に。マルチ Player 戦闘の挙動調律に効く。
- editor で AI param (`MeleeConfig`) を編集する UI は Phase 1 後 (character editor の
  別タブとして追加)。Phase 1 は手書き YAML のみ。
- Brain trait 化は Phase 2 で `AllyBrain` を実装したときに `MeleeBrain` との共通項を見て判断。

**ランタイムすり替え対応 (段階的)**

想定 use case: (a) debug/replay (Player を bot に置換 / Enemy を手動操作)、(b) シナリオでの
Ally 加入離脱 (Enemy → Ally → Enemy)、(c) Mind Control 系能力 (戦闘中の中身入れ替え)。

本 ADR の 3 層分離により、**Brain component の add/remove だけで部分的なすり替えは可能**:

- Player entity に `BotBrain` を attach + `PlayerInputController` を remove → Player auto 化
- Enemy entity に `PlayerInputController` を attach + `MeleeBrain` を remove → Enemy を手動操作

ただし現状の `Player(PlayerId)` / `Enemy` marker は「役割 (味方/敵)」と「操作主体 (人/AI)」を
**1 つの marker で兼ねている** ため、完全すり替え (use case (c)) では以下が破綻する:

- `camera_follow` (`With<Player>`) — Player marker を外すと camera 見失う
- `HUD` (ADR-0030 の `PlayerId` 引き) — Player marker を外すと HP bar 消える
- `sync_animation` (Resource 引き) vs `sync_enemy_animation` (component 引き) — Animation 供給源が違うので Player ↔ Enemy 化で animation が引けなくなる
- `LastEngagedWith` (Player 専用 component) — Enemy 化で意味を失う

**完全すり替え対応は Phase 4+ で marker 分離 ADR として別途扱う**。設計の方向性は決めておく:

- `Side: Hero | Villain` (= 当たり判定 / 勝敗の所属) を全 character に attach
- 「操作主体」は Brain component の種類 (`PlayerInputController` / `MeleeBrain` /
  `BotBrain` / `AllyBrain`) で表現
- `camera_follow` / HUD 系 query を `With<PlayerInputController>` ベースに書き換え
- Animation 供給を Resource / component の二系統から **一系統 (= 全 character が
  component で持つ)** に統一

Phase 1 はこの方向性を ADR に明記するに留め、use case (a)(b) は **Brain の add/remove +
必要なら entity 再 spawn** で扱う。use case (c) (完全な Mind Control) は marker 分離 ADR
完了後の Phase 4+ で実装。

## Phase 2 補追 (2026-06-27)

味方 NPC (`AllyBrain`) 実装時に確定した小さな設計判断。

### Marker 構成

`Player` / `Enemy` と並ぶ第 3 marker として `Ally` component を導入する。Phase 4 で予定
されている Side / Controller 分離までは、Enemy と並列に置く形で運用する。これによる影響:

- `camera_follow` / `PlayerInputController` (= `With<Player>`) からは自動で除外される
- `sync_enemy_animation` は `Or<With<Enemy>, With<Ally>>` に拡張して Ally の
  CharacterState 変化でも animation を hot swap させる (per-entity `EnemyAnimationSet`
  を Enemy と共用)。component 名は Phase 4 の marker 分離タイミングで rename を再検討
- `resolve_ally_attacks` (Ally→Enemy) を新規追加し、`resolve_player_attacks` (Player→Enemy)
  と並列で AttackSet::Resolve に乗せる (ADR-0036 と同形のフロー)。`LastEngagedWith` は
  Ally 側に持たない (= HUD engagement-link の起点にならない) ので Player 経路の attacker
  query から複製しただけの形。3 marker の disjoint を Bevy schedule checker に明示するため、
  既存の Player / Enemy 経路の attacker / victim query に `Without<Ally>` も追加した
  - **Enemy → Ally の damage は本 Phase ではやらない**: ADR-0036 が Player victim のみを
    対象にしているのと同じく、Phase 2 では Ally victim 経路 (= Enemy が Ally を殴る) を
    実装しない。Ally の `HitPoints` は将来用に attach されているが、現状減らない。完全な
    対称化は Phase 4 (marker 分離) と合わせて再評価する

### `AllyBrain` の FSM 設計

3 状態 `Follow / Chase / Attack`。意思決定は decision_interval ごとに以下の順:

1. **nearest Enemy 評価**: hysteresis 込みで chase 圏内 (Chase/Attack 中なら
   `chase_exit_range_px`、それ以外なら `chase_enter_range_px`) なら engage。Chase/Attack
   は `MeleeBrain` と同じヒステリシス + cooldown + dwell ガード。
2. **Enemy 不在 / 圏外**: nearest Player を target に Follow。
   - `follow_distance_min_px` 未満で停止 (move=0、CharacterState::Idle 相当)
   - `follow_distance_max_px` 超で追従再開
   - 間 (= dead zone) では `(min+max)/2` を境界に「距離が大きい側で follow を継続」近似
   - X 軸 dead zone は `follow_distance_min_px` を流用、Z 軸 dead zone は Melee の Chase
     と同じ `CHASE_Z_DEAD_ZONE_PX = 8px`

target 選定の **Enemy 優先 + 不在時のみ Player follow** という方針を採った。「距離閾値で
Player follow ⇄ Enemy attack を切り替える」案も検討したが、beat-em-up の挙動として
「視認した敵を放置して Player に貼り付く」のは違和感が大きいので Enemy 優先に倒した。

### `AllyConfig` パラメータ

Chase/Attack 系は `MeleeConfig` と同じ default 定数を共有する (`DEFAULT_AI_*`)。
追加で:

- `follow_distance_min_px = 40` (= キャラ 1 体ぶんの距離)
- `follow_distance_max_px = 80` (40 px hysteresis、1 decision で抜けるには十分な幅)

`AllyConfig` と `MeleeConfig` は重複フィールドが多いが、Brain 用の data type であって
trait 化や共通基底を切るのは [memory: dedup_later] (まだ 2 種類なので早すぎる)。
3 つ目の Brain が出てきたら共通項を見て判断。

### Spawn 経路

`Project.allies: Vec<String>` を追加し、battle 起動時に Player の隣 (`spawn_x - 40 * n`)
に spawn する。`Character.ai` が `Some(AiConfig::Ally(_))` でない character を allies に
列挙したら warn を吐いて skip する (= ally として宣言した character は ai: kind: ally を
持つべき制約を緩く維持)。

## Phase 3 補追 (2026-06-28)

Player 自動化 (`BotBrain`) 実装時に確定した小さな設計判断。

### Marker / 競合解決方針 (案 A 排他)

`Player` marker を持つ entity に `BotBrain` component を attach することで Player auto 化する。
**競合解決は案 A (排他)** を採用: `player_input_controller` の Query に `Without<BotBrain>` を
入れ、BotBrain attach 中は手動入力 system が自然に skip する。

> Decision §「ランタイムすり替え対応 (段階的)」では「BotBrain attach + PlayerInputController
> を remove」と書いたが、Phase 1/2 で `PlayerInputController` は名前付き component ではなく
> system に解体された経緯がある (= remove 対象が無い)。同じ排他効果を **system 側の Query
> filter** で実現したのが本 Phase の実装。「Brain component の add/remove で入力源を切り替える」
> 趣旨はそのまま (= `BotBrain` を attach すれば手動入力 system が透過的に skip する)。

排他方式 (案 A) を採った根拠:

- **2 system が同 entity の AiCommand を上書きする race** を構造的に避けられる
  (`Without<BotBrain>` filter で Bevy schedule checker に disjoint を強制)。
- `BotBrain` の remove で即手動に戻る可逆性を維持できる (component の add/remove で切替)。
- 候補だった案 B (生入力 1 frame 検出で BotBrain を一時 suspend) や案 C (移動は merge、攻撃は
  生入力優先) は実装複雑性で劣り、デモプレイ / オートバトル用途では merge / take-over の
  需要が薄い (= 入力源は丸ごと切替で十分)。Phase 4+ で demo 中の手動 takeover を要求が
  出てきた段階で再検討する。

### `BotBrain` の FSM

3 状態 `Idle / Chase / Attack`。意思決定の流れは:

1. nearest Enemy 評価 (`With<Enemy>` 限定 query、`nearest_with_pos` 流用)。
2. **Enemy 不在 / 圏外**: Idle に戻して AiCommand を全 0 (= Player はその場で立つ)。
   `AllyBrain` の `Follow` に相当する状態は無し (= Player は元々「立ち止まる」が default)。
3. Chase / Attack の hysteresis + dwell + cooldown は `MeleeBrain` と完全同型
   (`decide_bot_next_state` は `decide_next_state` の `BotState` 版コピー)。

target 選定は **最近 Enemy hardcode**。Phase 4 で `TargetSelector` enum (`Nearest` /
`LastAttacker` / `Random`) 化候補。

### Config: `MeleeConfig` を流用

Phase 3 では Bot 専用 Config は持たず、`MeleeBrain` と同じ `MeleeConfig` を流用する
(= 雑魚 Enemy と同じ反応速度 / 距離閾値で動く)。Bot 専用 param (perception_range /
panic_threshold / replay schedule 等) が必要になったら、Phase 4 で `BotConfig` を新設すれば
良い ([memory: dedup_later] 方針 — 3 Brain が揃ったあと trait 化 / Config 分離を再検討する)。

### 起動経路: env var `BEATEMUP_PLAYER_BOT`

`battle.rs::setup` の Player spawn 直後で `std::env::var("BEATEMUP_PLAYER_BOT")` を見て、
非空なら `BotBrain` を insert する (`BEATEMUP_PROJECT` / `BEATEMUP_RUNTIME_DIR` と平仄、
`is_some_and(|v| !v.is_empty())` で「空文字 = unset と同じ」扱い)。ローカル目視確認は
`BEATEMUP_PLAYER_BOT=1 just engine-run-sample` で起動。

character YAML 経由 (`ai: kind: bot`) は Phase 3 では実装しない:

- Player キャラの YAML を bot 化用に書き換える運用は sample-projects との相性が悪い
  (hero YAML を 1 つだけ持つ minimal project では切り戻し操作が増える)。
- character YAML から AI を切り替える表現は Phase 4 の Side/Controller marker 分離と一緒に
  入れるほうが筋が良い (= `Side` / `Controller` 軸でキャラ役割と操作主体を直交化したあと、
  Controller を YAML から指定する形)。

### dedup は見送り

`MeleeBrain` と `BotBrain` は FSM 遷移ロジック / cmd 生成 / cooldown 管理が高重複だが、
[memory: dedup_later] に従い Phase 3 では trait 化しない。Brain 3 種類 (Melee / Ally / Bot) が
出揃ったので、**Phase 4 着手前** に別 ADR (Brain trait 抽出 / TargetSelector enum 導入) で
共通基盤を扱う。

### 先送り事項 (Phase 4 以降)

- `AiConfig::Bot(BotConfig)` の YAML 化 (Phase 4 marker 分離と同時)
- Brain trait 抽出 (Melee / Ally / Bot の dedup)
- `TargetSelector` enum (`Nearest` / `LastAttacker` / `Random`)
- recorded replay brain / scripted demo brain (本 ADR Decision §「BotBrain (Phase 3): 2
  形態」で言及)
- Bot の Guard / Jump / DownAttack 自動使用 (現状は Attack のみで距離詰めと殴り)
- demo 中の手動 takeover (案 B / C を取り入れる場合)

## Phase 4 補追 (2026-06-28)

Side / Controller marker 分離 + Enemy → Ally 対称化 + `AiConfig::Bot` YAML 化 は
[ADR-0038](0038-side-controller-marker-separation.md) で実装した。本 ADR 内で先送りにしていた:

- 旧 `Player(PlayerId)` / `Enemy` / `Ally` marker は完全削除し、
  `Side: Hero | Villain` + `Controller: Human | Ai` の 2 enum component に直交化
- `MeleeBrain` の target を `Side::Hero` 全体に拡張 (= Enemy → Ally chase が動く)
- `attack.rs` の resolve 経路を 3 系統 → 2 系統 (`resolve_hero_attacks` /
  `resolve_villain_attacks`) に統合。Enemy → Ally の damage / knockback / KO が runtime
  レベルで対称化
- `AiConfig::Bot(BotConfig)` variant を追加、Hero character YAML の `ai: kind: bot` 経路を
  有効化。env var `BEATEMUP_PLAYER_BOT` との両立 (env 優先) で Phase 3 挙動の回帰なし

HUD ally HP bar / KO 演出 / engagement-link / icon HUD などの HUD 系対称化は本 Phase 4 で
扱わない (= ADR-0030/0031/0032/0033 の Phase 補追として別 Issue で実装)。

## Phase 5 補追 (2026-06-28): editor UI で `AiConfig` を編集可能にする (Issue #8)

ADR-0038 (Side / Controller marker 分離) + ADR-0039 (Brain trait + TargetSelector) で
`AiConfig` schema (3 variant × `EngagementConfig` 7 field + Ally の Follow 2 field +
`TargetSelector` 4 variant) が固まったので、editor (`packages/editor-desktop`) から
GUI で編集可能にする。

### 採用した UI パターン

`Character` 詳細画面の `Properties` エリア下に **`AI Brain` collapsible section** を 1 つ
追加し、`PhysicsSection` と同形の `dl` グリッドで以下を並べる:

- **Kind dropdown** (`<select>`): `(none)` / `Melee` / `Ally` / `Bot` の 4 択。none で
  `Character.ai = None`、それ以外で当該 variant の `default()` を入れる
- **Target Selector dropdown**: `TargetSelector` 4 variant (Random / WeightedByThreat は
  「(stub)」表記で UI 上明示するが選択 + 保存は可能 — ADR-0039 規約)
- **Engagement field grid**: `chase_enter/exit` / `attack_enter/exit` (f32 px、4 行) +
  `attack_cooldown` / `decision_interval` / `min_dwell` (u32 tick、3 行)
- **Follow field grid (Ally のみ)**: `follow_distance_min/max_px` (f32 px、2 行)

field 単位は `EditPhysicsF32Inline` / `EditPhysicsU32Inline` と同じ「✎ / Save / Cancel」の
inline 編集 pattern を踏襲。書き込みは `CharacterRepository::update_metadata` で
`{character_name}.yml` だけを差し替える (sprite-groups/ animations/ には触らない)。

### kind 切替時は新 variant の `default()` で初期化する (引き継ぎなし)

例: Melee → Ally に切り替えると `EngagementConfig` の値も含めて `AllyConfig::default()` で
書き直される (= 元の `chase_enter_range_px` 等は引き継がれない)。理由:

- Ally → Melee で `follow_distance_*` を silently 落とすことになる
- Brain ごとの調律値 (例: Bot は短い decision_interval が望ましい等) が混入して予測不能
- ユーザーが「同じ値を引き継ぎたい」場合は kind 切替前に値をメモ → 切替後に再入力で済む

### editor 側 `DEFAULT_AI_*` の二重定義

editor の `entities/character/model.rs` に **engine と同値**の `DEFAULT_AI_*` 定数群を
再定義する。本プロジェクトの規約 (= editor / engine は独立、共通化は両者が熟れてから探す、
[memory: dedup_later]) に従い、shared crate に切り出すのは見送る。default 値の同期は ADR-0035
本文の数値 (Phase 1.1 で hard-code、Phase 1.2 で YAML 化したもの) に揃え、engine 側変更時は
本 ADR の数値と editor / engine の定数 3 箇所を同時に更新する規約とする。

### Yaranai (Phase 5 では扱わない)

- **preview / hot-reload** — editor で値を変えたら runtime engine にも live 反映、は別 Issue
  送り (data-flow.md 参照)。本 Phase 5 では「YAML を編集 → engine を再起動」が前提
- **新 Character 作成時の AI 既定 attach** — `CreateCharacterButton` 経由の create では
  `ai: None` を維持。AI が要るキャラは編集画面から後付けで設定する
- **stub variant の挙動実装** — `Random` / `WeightedByThreat` を選んで保存自体は可能だが、
  engine 側挙動は引き続き warn + Nearest fallback (ADR-0039 規約)
- **OOUI 的 wizard** — 「kind を選ぶと適切な default が候補表示される」等の補助 UI は不要、
  単純 dropdown + flat field grid で足りる

## 関連

- [ADR-0030](0030-multi-player-hud-targeting.md): `Player(PlayerId)`、`ActionMap` の Player
  ごと分離余地。`PlayerInputController` の入力ソースは将来 `HashMap<PlayerId, ActionMap>` で
  Player 別に持つ。
- [ADR-0031](0031-enemy-hp-bar-and-engagement-tracking.md): `LastEngagedWith` (Player→Enemy)。
  逆方向 (`MeleeBrain.target`) は本 ADR で導入する。`TargetSelector::LastAttacker` で
  `LastEngagedWith` を逆引きする案あり。
- [ADR-0024](0024-knockback-gauge-attackboxmeta-driven-hit.md): 攻撃解決の流れ。AI の attack
  発火は `AiCommand.attack = true` で `apply_command` が `CharacterState::Attack` に遷移、
  以降は既存フローに乗る。
- [ADR-0027](0027-jump-and-aerial-combat.md): Jump / JumpAttack 系の優先度。`apply_command`
  はこの優先度をそのまま引き継ぐ。
- [ADR-0028](0028-guard-gauge-and-guard-break.md): Guard の押下継続。`AiCommand.guard` は
  pressed 相当 (押下中 true) で表現。
- [ADR-0034](0034-frame-sound-hit-guard-routing.md): `AttackOutcome` は attacker 側に
  attach する設計。AI attacker にもそのまま attach すれば Hit/Guard SE の出し分けが効く。
- `packages/engine/src/features/character/ai.rs` — 本 ADR の実装ファイル
- `packages/engine/src/features/character/movement.rs:110` — `handle_input` を割って
  `PlayerInputController` + `apply_command` に分割する起点
- `packages/engine/src/features/character/state_machine.rs:30` — `CharacterState` / `is_locked()`
- `packages/engine/src/features/character/state_debug.rs` — debug overlay に AI 行を追加
- `packages/engine/src/entities/character/model.rs` — `Character.ai: Option<AiConfig>` 追加
- `packages/engine/src/scenes/battle.rs` — Player spawn 直後で `BEATEMUP_PLAYER_BOT` 検出 →
  `BotBrain::new(MeleeConfig::default())` insert (Phase 3)
