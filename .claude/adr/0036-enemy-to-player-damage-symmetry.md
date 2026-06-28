# ADR-0036: Enemy → Player の damage / knockback を attacker-agnostic で実装する

## Status

Proposed (2026-06-27 ドラフト、Issue #2 で議論中)

## Context

ADR-0035 で MeleeBrain (Enemy 用近接 AI) が入り、grunt が Player を追って近接 Attack
モーションを出すようになった。ところが **Enemy の Attack は Player に damage を与えない**:
被弾側 (Player) の HP / knockback ゲージは一切減らない、knockback も発動しない、Hit/Guard SE
も鳴らない。

これは ADR-0034 (Frame.sound Hit/Guard routing) に明記された future work:

> `AttackOutcome は attacker (Player) のみ。Enemy には attach しない (= Enemy attack の
> 出し分けは future work)`

ADR-0035 Consequences の Phase 1.3 でも「Enemy → Player の damage を埋める」を後続作業として
切り分けている。

現状の asymmetry を整理すると:

| 項目 | Player | Enemy |
|---|---|---|
| `HitPoints` / `BodyBox` / `Combatant` / `PhysicsParams` / `KinematicVel` / `CharacterDepth` | 両方に attach | 両方に attach |
| `AttackOutcome` (ADR-0034) | spawn 時に attach | **無し** |
| `AttackHitConsumed` (1 attack 1 hit フラグ) | spawn 時に attach | **無し** |
| `LastEngagedWith` (ADR-0031 / HUD engagement-link) | spawn 時に attach | 無し (= 設計上 Player のみ) |
| `resolve_hits` 経路 | Player → Enemy の query 固定 | **未対応** |
| `reset_attack_hit_on_attack_start` | `With<Player>` で gating | **対象外** |
| `is_attack_hit_active` で AttackBox 生成 | Player Attack frame で発火 | 関数自体は role 不問だが呼び出し側 (`resolve_hits` / `hitbox_debug`) が Player のみ |
| `hitbox_debug` AttackBox overlay | Player のみ赤枠 | **無し** |

下半分 (`AttackOutcome` 以下) を Enemy にも揃えれば、ADR-0024 (knockback) / ADR-0028 (guard) /
ADR-0034 (sound routing) の既存フローを attacker / victim 役割そのままに Enemy → Player 方向で
再利用できる。下半分の component / system はいずれも attacker と victim を marker に依存して
gating している部分が薄く、対称化のコストは小さい。

multi-player (ADR-0030) の 4 人 co-op を考えると、`AttackHitConsumed` は **attacker 単位** で
持つフラグ (= 1 swing で同 victim を多重 hit しない) なので、attacker が Player か Enemy か
問わず attach すれば自然に動く。被弾側 query は victim marker (Player / Enemy) で絞る。

## Decision

### スコープ (この ADR で決めること)

- Enemy → Player の damage / knockback / hitstop / SE routing / guard 削りを Phase 1.3 として
  通す。Player → Enemy 経路と「役割の対称」になるところまで。
- friendly-fire (Player ⇒ Player, Enemy ⇒ Enemy) はこの ADR では拒否する。実装は marker
  hardcode (= 被弾側 query を `With<Enemy>` / `With<Player>` で排他に書く) で済ませる。

### スコープ外 (別 ADR / 別 issue)

- **Side / Faction marker (Hero / Villain)**: ADR-0035 Consequences の Phase 4+ メモにある
  汎用化。ここで `Side` を入れると friendly-fire の許可分岐や 3 陣営化が綺麗になるが、
  本 ADR の射程外。マーカ追加は別 ADR で行う。
- **Player の死亡 / ゲームオーバー UI**: Phase 1.3 では Enemy と対称の挙動
  (`final_action = Dead` → `advance_stage_timer` が LieDown 到達時に永続停止) を流用し、
  ゲームオーバー画面 / リトライ / co-op の蘇生は別 issue で扱う。永続停止状態の Player は
  以後操作を受け付けないが、Enemy からの追撃 (down_hit) は既存ルールで継続する。
- **Enemy → Player の `LastEngagedWith` 相当**: HUD は Player 起点の engagement-link
  (Player が殴った最後の enemy を target にする) を前提に作られている。逆方向 (Player を
  最後に殴った Enemy を覚える) は今のところ HUD で参照箇所が無く、未実装のままにする。
  必要になったら別 ADR で `LastAttackedBy` 等を追加する。

### `AttackOutcome` / `AttackHitConsumed` を両 marker に attach する

`scenes/battle.rs:spawn_opponents_on_trigger` (Enemy spawn) の `commands.entity(e).insert(...)`
連鎖に以下を追加:

```rust
.insert(AttackOutcome::default())
.insert(AttackHitConsumed::default())
```

両 component とも default = "未消費 / Idle" で開始するので、Enemy 側に追加するだけで
attacker 系 system が自然に Enemy entity を拾える。

ADR-0034 の `track_animation_swap` (Idle 復帰系) は既に `Option<&mut AttackOutcome>` で
書き込み対象を持っているため、Enemy に AttackOutcome を attach しても追加変更不要 (関数内で
attacker marker を見ていない)。

### `resolve_hits` を 2 system に分けて inner helper を共通化

attacker / victim の component セットは同じ (= 上の表の下段以外は両 marker に attach
されている) なので、現 `try_apply_attack_to_enemy` を attacker-victim 非依存な
`try_apply_attack_to_victim` にリネームし、以下 2 system でそれぞれ呼ぶ:

```rust
fn resolve_player_attacks(
    mut commands: Commands,
    mut attacker_query: Query<AttackerComponents, (With<Player>, Without<Enemy>)>,
    mut victim_query: Query<VictimComponents, (With<Enemy>, Without<Player>)>,
    player_entity_query: Query<Entity, With<Player>>,
) { ... }

fn resolve_enemy_attacks(
    mut commands: Commands,
    mut attacker_query: Query<AttackerComponents, (With<Enemy>, Without<Player>)>,
    mut victim_query: Query<VictimComponents, (With<Player>, Without<Enemy>)>,
    // Enemy attacker entity を attacker 側 hit_stop ターゲットとして使う
    enemy_entity_query: Query<Entity, With<Enemy>>,
) { ... }
```

両 system とも `AttackSet::Resolve` SystemSet に入れる。ADR-0034 の `tick_sound_dispatch`
順序保証 (`.after(AttackSet::Resolve)`) はそのまま成立する (両 attacker 系の attack_outcome
書き込み確定 → SE 出し分け、の順)。

**1 system に attacker query を 2 つ持つ案** は採用しない:

- Bevy の `Query` は 1 system 内で同 component に対する `&mut` を 2 query に分けるとき
  marker disjoint を `Without` で明示する必要があり、Player attacker query / Enemy victim
  query / Enemy attacker query / Player victim query の 4 つを並べると静的解析の組み合わせが
  煩雑になる
- 1 system 1 方向に分けるとロジックの読み手が attacker / victim の役割を取り違えにくい
- 共通の inner helper (`try_apply_attack_to_victim`) で実体は重複しない

**attacker / victim 共通 query にして 1 system 1 ループにする案** (e.g. `With<Combatant>`
で全 fighter を取って 2 重ループ) は採用しない:

- Bevy で同 component に対する `&mut` を 2 重ループで取り出すのは禁止 (borrow check で fail)。
  `iter_combinations_mut` を使えば形式上は書けるが、`HitPoints` / `CharacterState` /
  `Combatant` / `KinematicVel` 等 attacker と victim の component セットが対称でない
  (attacker は AttackOutcome / AttackHitConsumed を `&mut`、victim は HitPoints などを `&mut`)
  ため、`iter_combinations_mut` の同一型タプル制約と合わない
- friendly-fire 拒否を marker レベルで強制したい (= hardcode を物理的に書けるようにする)

### `reset_attack_hit_on_attack_start` の Player gating を外す

```rust
fn reset_attack_hit_on_attack_start(
    mut query: Query<(&CharacterState, &mut AttackHitConsumed), Changed<CharacterState>>,
) { ... }
```

`AttackHitConsumed` を持つ entity 全てが対象なので、Player と Enemy 両方の attacker が
新しい attack に入った瞬間に false にリセットされる。

### `hitbox_debug.rs` の AttackBox overlay を対称化

`draw_hitboxes` の player_query の `With<Player>` filter を外して `attacker_query` 化し、
`AnimationFrames` + `CharacterState` + `CharacterDepth` + `WorldPosition` + `Facing` を持つ
entity 全てに対して `is_attack_hit_active` を判定する (両 marker 対象)。Enemy の AttackBox も
F1 overlay で赤枠が見える。

### system 順序

```
PlayerInputController / Brain → apply_command
                                    ↓ (CharacterState 確定)
sync_body_box → reset_attack_hit_on_attack_start
                                    ↓
                              AttackSet::Resolve {
                                resolve_player_attacks,
                                resolve_enemy_attacks,
                              }
                                    ↓ (AttackOutcome 確定)
                              tick_sound_dispatch
```

2 system は同 `AttackSet::Resolve` に入れて順序不問にする (`Query` の借用は disjoint
= Player attacker / Enemy victim vs Enemy attacker / Player victim で交わらないので
Bevy 側で並列実行できる)。

### Player の被弾死亡時の挙動

- `HitPoints.current = 0` で `final_action = Dead`、`advance_stage_timer` が LieDown 到達時に
  永続停止 (Enemy と同じ既存フロー)
- Player entity は **despawn しない** (Enemy の致命傷も同様、ADR-0024 Phase B では despawn
  していない。LieDown で残骸として残る)。これにより HUD の Player HP bar 等は entity 参照を
  失わない
- ゲームオーバー UI / リトライ / co-op 蘇生は別 issue (Phase 1.3 範囲外)
- 永続停止状態の Player には Enemy の attack が引き続き当たる (= 倒れた Player への追撃)。
  既存の `down_hit_count` cap で過剰連打は防がれる

## Alternatives Considered

- **Side marker (Hero / Villain) を本 ADR で導入する**:
    - friendly-fire 拒否を marker レベルで宣言的に書けて、3 陣営化や mind-control 系演出
      (一時的に陣営切り替え) の拡張余地が出る
    - Phase 1.3 のスコープを越える設計判断 (`PlayerId` との関係、save data、AI Brain の
      target 解決) を巻き込み、本 ADR が肥大化する
    - 別 ADR で改めて入れたほうが議論しやすい
- **attacker / victim を 1 経路に統合する (`With<Combatant>` で全 fighter loop)**:
    - 「Decision」節で却下 (Bevy の借用 / `iter_combinations_mut` 制約)
- **`AttackOutcome` を `Combatant` のフィールドに含める**:
    - ADR-0034 の Alternatives で「semantic がズレる」と却下されている。再検討せず踏襲
- **Player 死亡で entity を despawn する**:
    - HUD の Player HP bar の entity 参照が dangle する。`Query::get` の error 経路で対処
      する手もあるが、Enemy も despawn せず LieDown 残骸で表現している既存方針と非対称
- **Phase 1.3 で `LastAttackedBy` を Enemy → Player 方向にも追加する**:
    - HUD で参照する場所が無いので「書くだけで読まれない」死んだ component になる。
      必要になってから追加するほうが YAGNI

## Consequences

**得られたもの**

- Enemy → Player の damage / knockback / hitstop / SE routing / guard 削りが Player → Enemy
  経路と同じ仕組みで動く
- multi-player 4 人 co-op で「どの Player に Enemy attack が当たったか」も既存の victim query
  (`With<Player>`) で自然に解決される
- `try_apply_attack_to_victim` の signature が attacker / victim 非依存になるため、後で
  Side marker / Ally NPC が入っても inner helper は再利用できる
- ADR-0034 の `AttackOutcome` 設計が attacker-agnostic だった (= `&mut AttackOutcome` を
  Player に絞らず取れる構造) ことが想定通り効いた

**支払うコスト**

- `resolve_hits` が 2 system に増える (`resolve_player_attacks` / `resolve_enemy_attacks`)。
  実体ロジックは `try_apply_attack_to_victim` に集約するので重複は最小
- Enemy spawn bundle に 2 component (`AttackOutcome`, `AttackHitConsumed`) 追加分の
  memory コスト (1 entity あたり数 byte、無視できる規模)
- `hitbox_debug` の F1 overlay で Enemy の AttackBox も表示されるため、grunt 連打中の画面が
  赤枠だらけになる可能性。デバッグ用なので許容する

**今後の拡張余地**

- Side marker / Faction が入ったら、Player⇔Enemy hardcode の hit_query gating を
  `With<Side::Hero>` ⇔ `With<Side::Villain>` に置き換えるだけで mind-control や 3 陣営化が
  実現できる
- Player → Player friendly-fire を許可したくなったら、`resolve_player_attacks` の victim_query
  に `With<Player>` を許す分岐を追加する余地がある (現状は `With<Enemy>` hardcode で物理的に
  弾いている)
- Game Over / リトライ UI を Phase 4+ で被弾死亡 Player の検出 (`HitPoints::is_dead()` +
  `Player` marker + `FinalAction::Dead`) から起こせる
