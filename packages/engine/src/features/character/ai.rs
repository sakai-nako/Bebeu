//! AI feature (FSD: feature slice、ADR-0035 + ADR-0039)。
//!
//! ADR-0035 の 3 層分割 (Brain / Intent / Actuator) を本ファイルに集約する:
//!
//! - **Brain**: [`MeleeBrain`] (Villain 用 / Phase 1.1) / [`AllyBrain`] (Phase 2) /
//!   [`BotBrain`] (Phase 3、Hero 自動化)。FSM 3 状態 (Idle/Chase/Attack または Follow/Chase/Attack)。
//!   ADR-0039 で **共通基盤 (`EngagementState` / [`BrainCounters`] / [`Brain`] trait /
//!   [`decide_engagement`] / [`engagement_command`] / [`apply_attack_cooldown`])** を抽出し、
//!   3 Brain で意思決定ロジックを共有する。Brain ごとの差は `target query` (Side フィルタ) と
//!   AllyBrain の Follow フェーズだけ。
//! - **Intent**: [`AiCommand`] — per-entity component。Player の生入力と等価な「意図」表現
//!   (desire ベース)。[`super::movement::PlayerInputController`] や Brain が毎 frame 書き込む。
//! - **Actuator**: [`apply_command`] — `AiCommand` を `CharacterState` / `KinematicVel` /
//!   `Facing` / `WorldPosition` に焼き込む。Guard > Jump > Attack > DownAttack > 移動 の
//!   優先度判定と `is_locked()` skip を 1 箇所に集約する。

use bevy::prelude::*;

use crate::entities::character::{AllyConfig, EngagementConfig, MeleeConfig, TargetSelector};
use crate::entities::level::Level;

use super::animation::VSYNC_TICK_SECS;
use super::debug_control::SimulationSet;
use super::knockback::{KinematicVel, PhysicsParams};
use super::movement::{Controller, Facing, Side, WorldPosition, step_axis_aware};
use super::state_machine::CharacterState;

/// 1 秒あたりの移動量 (画像 px)。元 `movement::handle_input` から移植 (ADR-0035 で Actuator に集約)。
/// Beat 'em up のキャラ歩行はだいたい 60-100 px/sec、後で Character.physics 由来にする想定で
/// 現状は定数。
///
/// **60Hz pixel-perfect 補足**: 60 の整数倍 (60, 120, 180 ...) は毎 frame の snap step が
/// 「常に同じ px 数」になって完全に滑らかに見える。非整数倍 (例: 80 = 1.333 px/frame) は snap
/// pattern が `1, 2, 1, 1, 2 ...` の 3-frame 周期になるが、`AnimationSet::Tick` 順序整理後は
/// この程度の周期パターンは体感的に許容できる。歩行速度の見た目を優先したい場合は 80 等の値も
/// 使ってよい。
const MOVE_SPEED_PX_PER_SEC: f32 = 80.0;

/// `AiCommand.move_x` の絶対値がこれ未満では `Facing` を更新しない (ADR-0035 チャタリング防止)。
/// Player は [`super::movement::PlayerInputController`] が ±1.0 / 0 のデジタル値で出すので
/// 影響無し、AI 側で continuous な move_x を出した場合の左右ピクピクを防ぐ。
const FACING_DEAD_ZONE: f32 = 0.15;

/// Chase で詰める X 距離の下限を attack 発火距離より少し外側に取る (重なり防止)。`0.85` は
/// 「Attack 発火 (= `attack_enter_range_px`) 直前で立ち止まる」位置。
const CHASE_X_STOP_RATIO: f32 = 0.85;
/// Chase で詰める Z 距離の dead zone (奥行き合わせの余裕、px)。1 decision 周期で進む距離
/// (`MOVE_SPEED_PX_PER_SEC * VSYNC_TICK_SECS * decision_interval_ticks` ≒ `80/60*6` = `8 px`)
/// より小さいと dead zone 出入りで振動する。`8 px` は collider 1 マス前後で、視覚的にも
/// beat-em-up の「同じ列に居る」許容範囲。
const CHASE_Z_DEAD_ZONE_PX: f32 = 8.0;

/// per-entity の意図表現 (ADR-0035 Intent 層)。Player / AI で共通スキーマ (desire ベース)。
///
/// - `move_x` / `move_z`: -1.0..=1.0 (normalized)。Actuator 側で `MOVE_SPEED_PX_PER_SEC * dt`
///   をかけて実 px に直す。
/// - `attack` / `down_attack` / `jump`: 「したい間 true」(desire)。Actuator が
///   `is_locked()` skip で edge を暗黙生成するので、AI Brain は edge 管理を自前でやらない。
/// - `guard`: 押下中継続 (pressed 相当)。Guard 維持に必要。
/// - `face`: 強制向き。`None` なら `move_x` の符号で更新 (dead zone 以下では維持)。
///
/// `attack` / `down_attack` / `jump` / `guard` の 4 bool は意味的に独立した「現 frame で
/// したい入力」を表しており、state machine / enum 化は当てはまらない (`struct_excessive_bools`)。
#[allow(clippy::struct_excessive_bools)]
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AiCommand {
    pub move_x: f32,
    pub move_z: f32,
    pub attack: bool,
    pub down_attack: bool,
    pub jump: bool,
    pub guard: bool,
    pub face: Option<Facing>,
}

/// AI 系 system の順序枠 (ADR-0035 の Brain → Intent → Actuator 内、Intent → Actuator)。
///
/// - `ReadInputs`: AiCommand を書き込む側 (`PlayerInputController` / Brain) を入れる
/// - `Apply`: `apply_command` (AiCommand を CharacterState 等に焼く)
///
/// `Apply` は `ReadInputs` の後に走るよう chain される。`movement::sync_*` /
/// `camera_follow` は `.after(AiSet::Apply)` で順序付ける。
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AiSet {
    ReadInputs,
    Apply,
}

pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (AiSet::ReadInputs, AiSet::Apply)
                .chain()
                .in_set(SimulationSet::Active),
        )
        .add_systems(Update, melee_brain_tick.in_set(AiSet::ReadInputs))
        .add_systems(Update, ally_brain_tick.in_set(AiSet::ReadInputs))
        // ADR-0035 Phase 3: Player 自動化 Brain。`player_input_controller` と同じ
        // `ReadInputs` スロット (= AiCommand 書き込み層) に同居させる。両者は
        // `Without<BotBrain>` で marker レベル disjoint。
        .add_systems(Update, bot_brain_tick.in_set(AiSet::ReadInputs))
        .add_systems(Update, apply_command.in_set(AiSet::Apply));
    }
}

// ──────────────── Common Brain infrastructure (ADR-0039) ────────────────

/// 3 Brain で共有する Idle / Chase / Attack の意思決定 state (ADR-0039)。
/// `MeleeBrain` (Villain) と `BotBrain` (Hero 自動化) はこの enum をそのまま `state` field に
/// 持つ。`AllyBrain` は Follow フェーズが追加で必要なので [`AllyState`] を別途持ち、Engagement
/// 中の遷移だけ本 enum と相互変換する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngagementState {
    /// target 候補不在 / 圏外、または Ally の Follow に相当する「engagement していない」状態。
    /// AllyBrain では `AllyState::Follow` がこの位置を占める。
    #[default]
    Idle,
    /// target を捕捉して距離を詰めている状態。
    Chase,
    /// target が attack 圏内 + cooldown 0 のため攻撃を出している状態。
    Attack,
}

/// Brain が共通で持つカウンタ群 (ADR-0039)。decision_interval / dwell / cooldown の管理を
/// 1 つの struct に集約し、`*_brain_tick` 系 system では [`tick_counters`] と
/// [`apply_attack_cooldown`] helper 経由で操作する。
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainCounters {
    /// 現 state に入ってからの経過 tick (dwell time 判定用、遷移時に 0 リセット)。
    pub frames_since_state_entered: u32,
    /// 0 のとき decision を回し、回したあと `decision_interval_ticks` を再充填する。
    pub frames_until_next_decision: u32,
    /// Attack 発火後に `attack_cooldown_ticks` を入れ、0 まで countdown 中は `attack: false`
    /// を返す。Brain は `CharacterState` を見ないので、cooldown は Brain 内で完結させる。
    pub attack_cooldown_remaining: u32,
}

/// Brain (Melee/Ally/Bot) が満たす共通契約 (ADR-0039)。`*_brain_tick` の重複削減のため、
/// `BrainCounters` と `EngagementConfig` と `TargetSelector` への uniform アクセスだけを
/// trait 化する (target query / state 遷移 / Follow フェーズの分岐は Brain 側に残す)。
pub trait Brain {
    /// 共通 FSM パラメータ (= `EngagementConfig`) への参照。
    fn engagement(&self) -> &EngagementConfig;
    /// target 選定戦略 (= `TargetSelector`)。
    fn selector(&self) -> TargetSelector;
    /// per-Brain の counter 群への mutable 参照。
    fn counters_mut(&mut self) -> &mut BrainCounters;
    /// per-Brain の counter 群への参照 (debug overlay 等の readonly 用途)。
    fn counters(&self) -> &BrainCounters;
    /// 現 target entity (Nearest / LastEngaged の継続判定で使う)。
    fn target(&self) -> Option<Entity>;
    /// target entity を書き換える (`select_target` から呼ばれる)。
    fn set_target(&mut self, target: Option<Entity>);
}

// ──────────────── Helpers (ADR-0039) ────────────────

/// `BrainCounters` を 1 frame 進めつつ「今 frame で decision を回すか」を返す helper
/// (ADR-0039)。`true` → 呼び出し側が意思決定 + state 遷移を実行する、`false` → skip
/// (前回 decision の `AiCommand` をそのまま生かす)。
///
/// 副作用: `frames_since_state_entered` を +1、`attack_cooldown_remaining` を -1
/// (0 で饱和)、`frames_until_next_decision` を -1 (0 で饱和)。decision を回す frame では
/// `frames_until_next_decision` を `decision_interval_ticks` で再充填する。
fn tick_counters(counters: &mut BrainCounters, decision_interval_ticks: u32) -> bool {
    counters.frames_since_state_entered = counters.frames_since_state_entered.saturating_add(1);
    if counters.attack_cooldown_remaining > 0 {
        counters.attack_cooldown_remaining -= 1;
    }
    if counters.frames_until_next_decision > 0 {
        counters.frames_until_next_decision -= 1;
        return false;
    }
    counters.frames_until_next_decision = decision_interval_ticks;
    true
}

/// Attack 発火後の cooldown を仕込み、cooldown 中の `cmd.attack` を false に塗る helper
/// (ADR-0039)。順序の invariant: **Attack 進入 frame の `cmd.attack: true` を残す** ために、
/// この helper は `*cmd = ai_command_for(...)` の **後** に呼ぶ。
///
/// 過去バグ (Phase 1.1): cooldown を「state 遷移ブロック内」で先に仕込んだため、進入 frame の
/// `attack: true` が直後の `cooldown > 0` 分岐で false に塗られ、apply_command まで届かなかった。
/// この helper に hidden invariant を閉じ込めることで 3 Brain 共通の再発防止になる。
fn apply_attack_cooldown(
    counters: &mut BrainCounters,
    cmd: &mut AiCommand,
    entering_attack: bool,
    attack_cooldown_ticks: u32,
) {
    if entering_attack {
        counters.attack_cooldown_remaining = attack_cooldown_ticks;
    } else if counters.attack_cooldown_remaining > 0 {
        cmd.attack = false;
    }
}

/// 距離 + 現 state + dwell ガード + cooldown から次 [`EngagementState`] を決める (ADR-0039)。
/// ヒステリシスの 2 段閾値で「Chase ↔ Idle」「Attack ↔ Chase」境界に dead zone を作る
/// (ADR-0035 軸 1)。Idle からの最初の遷移は dwell 待たない (= 起動時に即 Chase に出られる)。
///
/// MeleeBrain / BotBrain では state 自身がこの enum なので直接呼ぶ。AllyBrain では
/// `AllyState::{Follow,Chase,Attack}` ↔ `EngagementState::{Idle,Chase,Attack}` の相互変換を
/// 経由する (`Follow` は engagement 上「Idle」相当として decide に渡す)。
fn decide_engagement(
    current: EngagementState,
    dist: f32,
    can_transition: bool,
    eng: &EngagementConfig,
    cooldown: u32,
) -> EngagementState {
    match current {
        EngagementState::Idle => {
            if dist > eng.chase_enter_range_px {
                EngagementState::Idle
            } else if dist <= eng.attack_enter_range_px && cooldown == 0 {
                EngagementState::Attack
            } else {
                EngagementState::Chase
            }
        }
        EngagementState::Chase => {
            if !can_transition {
                EngagementState::Chase
            } else if dist > eng.chase_exit_range_px {
                EngagementState::Idle
            } else if dist <= eng.attack_enter_range_px && cooldown == 0 {
                EngagementState::Attack
            } else {
                EngagementState::Chase
            }
        }
        EngagementState::Attack => {
            if !can_transition {
                EngagementState::Attack
            } else if cooldown > 0 || dist > eng.attack_exit_range_px {
                // cooldown 中は再度 Attack に居続けない (Chase に戻して距離詰めし直す)。
                EngagementState::Chase
            } else {
                EngagementState::Attack
            }
        }
    }
}

/// `EngagementState` と target 位置から `AiCommand` を生成する (ADR-0039)。Chase は target
/// 方向の単位ベクトル (詰めすぎ防止のため X/Z 軸別 dead zone)、Attack は `attack: true` のみ
/// (足を止めて殴る挙動)、Idle は all-zero (停止)。
///
/// Chase の dead zone:
/// - **X 軸**: `attack_enter_range_px * CHASE_X_STOP_RATIO` 以内なら move_x = 0。Player と
///   Enemy が重なるのを防ぎ、AttackBox の facing 方向に Player を捉えた位置で立つ。
/// - **Z 軸**: `CHASE_Z_DEAD_ZONE_PX` 以内なら move_z = 0。奥行きは数 px の余裕で揃える。
fn engagement_command(
    state: EngagementState,
    self_pos: &WorldPosition,
    target_pos: &WorldPosition,
    eng: &EngagementConfig,
) -> AiCommand {
    match state {
        EngagementState::Idle => AiCommand::default(),
        EngagementState::Chase => chase_command(self_pos, target_pos, eng.attack_enter_range_px),
        EngagementState::Attack => AiCommand {
            attack: true,
            ..AiCommand::default()
        },
    }
}

/// X stop ratio + Z dead zone を踏まえた「target を詰める」方向の単位ベクトルを返す。
/// Melee の Chase と Ally の Chase が共有する。`attack_enter_range_px` は X stop ratio の
/// 基準距離 (これより内側に詰めない)。
#[allow(clippy::similar_names)] // dx_eff / dz_eff は 2D 軸対の慣用、可読性を優先する。
fn chase_command(
    self_pos: &WorldPosition,
    target_pos: &WorldPosition,
    attack_enter_range_px: f32,
) -> AiCommand {
    let dx = target_pos.x - self_pos.x;
    let dz = target_pos.z - self_pos.z;
    let x_stop = attack_enter_range_px * CHASE_X_STOP_RATIO;
    let dx_eff = if dx.abs() > x_stop { dx } else { 0.0 };
    let dz_eff = if dz.abs() > CHASE_Z_DEAD_ZONE_PX {
        dz
    } else {
        0.0
    };
    let len = dx_eff.hypot(dz_eff);
    if len < f32::EPSILON {
        AiCommand::default()
    } else {
        AiCommand {
            move_x: dx_eff / len,
            move_z: dz_eff / len,
            ..AiCommand::default()
        }
    }
}

/// Follow 用の move 方向ベクトル。Chase と違い「Attack 発火距離」基準ではなく
/// 「Player との最小維持距離 (= `follow_distance_min_px`)」を X 軸 dead zone に使う。
/// Z は `CHASE_Z_DEAD_ZONE_PX` を共有 (奥行きを揃える基準は同じ)。
#[allow(clippy::similar_names)] // dx_eff / dz_eff は 2D 軸対の慣用、可読性を優先する。
fn follow_command(
    self_pos: &WorldPosition,
    target_pos: &WorldPosition,
    follow_min_px: f32,
) -> AiCommand {
    let dx = target_pos.x - self_pos.x;
    let dz = target_pos.z - self_pos.z;
    let dx_eff = if dx.abs() > follow_min_px { dx } else { 0.0 };
    let dz_eff = if dz.abs() > CHASE_Z_DEAD_ZONE_PX {
        dz
    } else {
        0.0
    };
    let len = dx_eff.hypot(dz_eff);
    if len < f32::EPSILON {
        AiCommand::default()
    } else {
        AiCommand {
            move_x: dx_eff / len,
            move_z: dz_eff / len,
            ..AiCommand::default()
        }
    }
}

fn distance_xz(a: &WorldPosition, b: &WorldPosition) -> f32 {
    let dx = b.x - a.x;
    let dz = b.z - a.z;
    dx.hypot(dz)
}

/// ADR-0038: `&Side` を query した候補集合の中から `side_filter` 一致 entity の最近接を返す。
/// `nearest_with_pos` の Side 値判定版 (`QueryFilter` だけで Side enum を絞り込めないため
/// 別 helper を切る)。`select_target` の Nearest 実装の中で使われる。
fn nearest_on_side<M: bevy::ecs::query::QueryFilter>(
    self_pos: &WorldPosition,
    side_filter: Side,
    candidates: &Query<(Entity, &WorldPosition, &Side), M>,
) -> Option<(Entity, WorldPosition)> {
    candidates
        .iter()
        .filter(|(_, _, s)| **s == side_filter)
        .map(|(e, p, _)| (e, *p, distance_xz(self_pos, p)))
        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(e, p, _)| (e, p))
}

/// Brain の target 選定 dispatcher (ADR-0039)。`TargetSelector` 値に応じて Nearest /
/// LastEngaged の挙動を分岐し、Random / WeightedByThreat は warn + Nearest フォールバック
/// (stub)。`current_target` には Brain 側の `brain.target` を渡す (= `LastEngaged` の継続
/// 判定で参照)。
///
/// LastEngaged の判定:
/// - `current_target == Some(e)` かつ `candidates.get(e)` で取得可能 (= 生存)
/// - 取得した `&Side` が `side_filter` と一致 (= 同 side に遷移していない)
///
/// 上記両方を満たすと前回 target を継続。それ以外は Nearest にフォールバックする。
/// AllyBrain で「Follow 中に Hero+Human を target にしていた状態」から engagement に
/// 切り替わったときも、`candidates` (= Villain query) からは Hero が見つからないため
/// 自動的に Nearest fallback に倒れる (= side mismatch で構造的に保護)。
fn select_target<M: bevy::ecs::query::QueryFilter>(
    self_pos: &WorldPosition,
    side_filter: Side,
    selector: TargetSelector,
    current_target: Option<Entity>,
    candidates: &Query<(Entity, &WorldPosition, &Side), M>,
) -> Option<(Entity, WorldPosition)> {
    match selector {
        TargetSelector::Nearest => nearest_on_side(self_pos, side_filter, candidates),
        TargetSelector::LastEngaged => {
            if let Some(target) = current_target
                && let Ok((_, pos, side)) = candidates.get(target)
                && *side == side_filter
            {
                return Some((target, *pos));
            }
            nearest_on_side(self_pos, side_filter, candidates)
        }
        TargetSelector::Random | TargetSelector::WeightedByThreat => {
            // ADR-0039: stub variant。実装は ボス級 hate 管理 / Random demo の実需が出た
            // ときに別 ADR + Issue で行う。warn_once! でログを 1 回だけ吐いて Nearest に
            // フォールバックすることで、YAML に書いた値が事故的に動いてしまう前にユーザーに
            // 気付かせる。
            bevy::log::warn_once!(
                "TargetSelector::{:?} is not implemented yet (ADR-0039 stub) — \
                 falling back to Nearest. Switch YAML to `selector: nearest` or \
                 `selector: last_engaged` to silence this.",
                selector,
            );
            nearest_on_side(self_pos, side_filter, candidates)
        }
    }
}

/// ADR-0038: AllyBrain の Follow target 用。`Side::Hero + Controller::Human` の中から
/// 最近接 entity を返す。`nearest_on_side` と統合できるが、Side / Controller の AND 条件は
/// 用途が限定的なので別 helper に切る。
fn nearest_hero_human<M: bevy::ecs::query::QueryFilter>(
    self_pos: &WorldPosition,
    candidates: &Query<(Entity, &WorldPosition, &Side, &Controller), M>,
) -> Option<(Entity, WorldPosition)> {
    candidates
        .iter()
        .filter(|(_, _, side, ctrl)| {
            matches!(side, Side::Hero) && matches!(ctrl, Controller::Human)
        })
        .map(|(e, p, _, _)| (e, *p, distance_xz(self_pos, p)))
        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(e, p, _)| (e, p))
}

// ──────────────── MeleeBrain (Villain) ────────────────

/// Villain 近接 AI Brain (ADR-0035 Phase 1.1 + ADR-0039)。FSM 3 状態 ([`EngagementState`])
/// + チャタリング防止の dwell ガード ([`BrainCounters`]) を担当する。
///
/// **Brain は `CharacterState` に直接触らない**。意思決定の結果は [`AiCommand`] に焼き、
/// 状態遷移は [`apply_command`] が担う (ADR-0035 3 層責務分割)。
#[derive(Component, Debug, Clone)]
pub struct MeleeBrain {
    pub state: EngagementState,
    pub target: Option<Entity>,
    pub counters: BrainCounters,
    pub config: MeleeConfig,
}

impl MeleeBrain {
    #[must_use]
    pub fn new(config: MeleeConfig) -> Self {
        Self {
            state: EngagementState::Idle,
            target: None,
            counters: BrainCounters::default(),
            config,
        }
    }
}

impl Brain for MeleeBrain {
    fn engagement(&self) -> &EngagementConfig {
        &self.config.engagement
    }
    fn selector(&self) -> TargetSelector {
        self.config.selector
    }
    fn counters_mut(&mut self) -> &mut BrainCounters {
        &mut self.counters
    }
    fn counters(&self) -> &BrainCounters {
        &self.counters
    }
    fn target(&self) -> Option<Entity> {
        self.target
    }
    fn set_target(&mut self, target: Option<Entity>) {
        self.target = target;
    }
}

/// Villain 用 `MeleeBrain` の tick (ADR-0035 Brain 層 / ADR-0038 / ADR-0039)。
///
/// 毎 frame 進める counter (`BrainCounters` の 3 つ) と、`decision_interval_ticks` で間引く
/// 意思決定本体 (target 再選定 + FSM 遷移 + AiCommand 書き込み) を分けて実装する。skip 中は
/// 前回 decision で書いた `AiCommand` がそのまま生き続け、Chase の移動方向は decision 間隔ぶん
/// 「古い target 位置」を向き続けるが、`decision_interval_ticks = 6` (約 100ms) なら体感問題ない。
///
/// ADR-0038: target は **`Side::Hero` 全体** (= 旧 Player + 旧 Ally 区別なし)。Brain owner 側は
/// MeleeBrain 持ち = Villain と暗黙に紐付くため、self に `&Side` を query する必要はない。
/// target query で `Without<MeleeBrain>` を入れて self との重なりを避ける。
fn melee_brain_tick(
    mut brains: Query<(&mut MeleeBrain, &WorldPosition, &mut AiCommand)>,
    hero_targets: Query<(Entity, &WorldPosition, &Side), Without<MeleeBrain>>,
) {
    for (mut brain, self_pos, mut cmd) in &mut brains {
        let decision_interval = brain.config.engagement.decision_interval_ticks;
        if !tick_counters(&mut brain.counters, decision_interval) {
            continue;
        }

        let selector = brain.config.selector;
        let prev_target = brain.target;
        let target = select_target(self_pos, Side::Hero, selector, prev_target, &hero_targets);
        brain.target = target.map(|(e, _)| e);

        let Some((_, target_pos)) = target else {
            // Hero 不在: Idle に戻して入力を切る。
            if brain.state != EngagementState::Idle {
                brain.state = EngagementState::Idle;
                brain.counters.frames_since_state_entered = 0;
            }
            *cmd = AiCommand::default();
            continue;
        };

        let dist = distance_xz(self_pos, &target_pos);
        let eng_cfg = brain.config.engagement.clone();
        let can_transition = brain.counters.frames_since_state_entered >= eng_cfg.min_dwell_ticks;
        let next_state = decide_engagement(
            brain.state,
            dist,
            can_transition,
            &eng_cfg,
            brain.counters.attack_cooldown_remaining,
        );
        let entering_attack =
            next_state == EngagementState::Attack && brain.state != EngagementState::Attack;
        if next_state != brain.state {
            brain.state = next_state;
            brain.counters.frames_since_state_entered = 0;
        }

        *cmd = engagement_command(brain.state, self_pos, &target_pos, &eng_cfg);
        apply_attack_cooldown(
            &mut brain.counters,
            &mut cmd,
            entering_attack,
            eng_cfg.attack_cooldown_ticks,
        );
    }
}

// ──────────────── AllyBrain (Hero NPC) ────────────────

/// 味方 NPC 用 Brain (ADR-0035 Phase 2 + ADR-0039)。FSM 3 状態 ([`AllyState`]) を持ち、
/// 「最も近い Enemy が chase 圏内にいれば Chase / Attack、そうでなければ Player を Follow」
/// の優先規約で意思決定する。
///
/// `MeleeBrain` と同じく Brain は `CharacterState` に直接触らない。意思決定の結果は
/// [`AiCommand`] に焼き、状態遷移は [`apply_command`] が担う (ADR-0035 3 層責務分割)。
#[derive(Component, Debug, Clone)]
pub struct AllyBrain {
    pub state: AllyState,
    /// 現在 engage 中の Enemy か follow 中の Player の entity (どちらかは `state` から分かる)。
    pub target: Option<Entity>,
    pub counters: BrainCounters,
    pub config: AllyConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllyState {
    /// Player に追従。Player との距離が `follow_distance_min/max` の hysteresis 帯に入ると
    /// 移動入力を切って Idle 相当 (= CharacterState::Idle) になる。
    /// engagement decision 上は [`EngagementState::Idle`] 相当として扱う。
    #[default]
    Follow,
    Chase,
    Attack,
}

impl AllyState {
    /// `decide_engagement` に渡す `EngagementState` への対応付け。Follow は「engagement 前」
    /// なので Idle に等価。
    fn to_engagement(self) -> EngagementState {
        match self {
            AllyState::Follow => EngagementState::Idle,
            AllyState::Chase => EngagementState::Chase,
            AllyState::Attack => EngagementState::Attack,
        }
    }

    /// `decide_engagement` の戻り値を AllyState に戻す (= Engagement 文脈なので Idle は
    /// 「engagement なし」を意味するが、Ally では既に Enemy 検出後に呼ばれるため Idle 戻りは
    /// 構造的に起きない — 防御的に Chase に倒す)。
    fn from_engagement(eng: EngagementState) -> Self {
        match eng {
            EngagementState::Idle | EngagementState::Chase => AllyState::Chase,
            EngagementState::Attack => AllyState::Attack,
        }
    }
}

impl AllyBrain {
    #[must_use]
    pub fn new(config: AllyConfig) -> Self {
        Self {
            state: AllyState::Follow,
            target: None,
            counters: BrainCounters::default(),
            config,
        }
    }
}

impl Brain for AllyBrain {
    fn engagement(&self) -> &EngagementConfig {
        &self.config.engagement
    }
    fn selector(&self) -> TargetSelector {
        self.config.selector
    }
    fn counters_mut(&mut self) -> &mut BrainCounters {
        &mut self.counters
    }
    fn counters(&self) -> &BrainCounters {
        &self.counters
    }
    fn target(&self) -> Option<Entity> {
        self.target
    }
    fn set_target(&mut self, target: Option<Entity>) {
        self.target = target;
    }
}

/// AllyBrain の tick (ADR-0035 Phase 2 / ADR-0038 / ADR-0039)。意思決定の流れは:
///
/// 1. nearest Villain を検索。chase 圏内 (= `chase_enter_range_px` or hysteresis 適用後の
///    `chase_exit_range_px`) ならその Villain に engage (Chase / Attack の hysteresis 判定を
///    [`decide_engagement`] に委譲)。
/// 2. Villain 不在 / 圏外なら nearest `Side::Hero + Controller::Human` (= Player) を target
///    に Follow。
/// 3. Player との距離が `follow_distance_min_px` 未満なら move=0 (= Idle 相当)、
///    `follow_distance_max_px` 超で再び詰めに行く (hysteresis)。
///
/// ADR-0038: Brain owner = AllyBrain 持ち (= Hero + Ai) なので Self の Side check は不要、
/// target query は `Without<AllyBrain>` で self を除外する。
#[allow(clippy::type_complexity)]
fn ally_brain_tick(
    mut brains: Query<(&mut AllyBrain, &WorldPosition, &mut AiCommand)>,
    side_targets: Query<(Entity, &WorldPosition, &Side), Without<AllyBrain>>,
    human_targets: Query<(Entity, &WorldPosition, &Side, &Controller), Without<AllyBrain>>,
) {
    for (mut brain, self_pos, mut cmd) in &mut brains {
        let decision_interval = brain.config.engagement.decision_interval_ticks;
        if !tick_counters(&mut brain.counters, decision_interval) {
            continue;
        }

        let eng_cfg = brain.config.engagement.clone();
        let can_transition = brain.counters.frames_since_state_entered >= eng_cfg.min_dwell_ticks;
        // ADR-0039: selector は **Villain engagement target** にのみ適用する。
        // Follow target (= nearest Hero+Human) は selector の対象外で常に Nearest。
        let selector = brain.config.selector;
        let prev_target = brain.target;
        let nearest_enemy = select_target(
            self_pos,
            Side::Villain,
            selector,
            prev_target,
            &side_targets,
        );
        let nearest_player = nearest_hero_human(self_pos, &human_targets);

        // 1) Enemy engagement: 最も近い Enemy が hysteresis 込みで chase 圏内なら engage。
        // 既に Chase/Attack で同じ target を追っていれば chase_exit_range_px、それ以外は
        // chase_enter_range_px を閾値にする。
        let enemy_decision = nearest_enemy.and_then(|(e, p)| {
            let dist = distance_xz(self_pos, &p);
            let already_engaged = matches!(brain.state, AllyState::Chase | AllyState::Attack)
                && brain.target == Some(e);
            let engagement_threshold = if already_engaged {
                eng_cfg.chase_exit_range_px
            } else {
                eng_cfg.chase_enter_range_px
            };
            (dist <= engagement_threshold).then_some((e, p, dist))
        });

        if let Some((enemy_entity, enemy_pos, dist)) = enemy_decision {
            brain.target = Some(enemy_entity);
            let next_eng = decide_engagement(
                brain.state.to_engagement(),
                dist,
                can_transition,
                &eng_cfg,
                brain.counters.attack_cooldown_remaining,
            );
            let next_state = AllyState::from_engagement(next_eng);
            let entering_attack =
                next_state == AllyState::Attack && brain.state != AllyState::Attack;
            if next_state != brain.state {
                brain.state = next_state;
                brain.counters.frames_since_state_entered = 0;
            }
            *cmd = engagement_command(brain.state.to_engagement(), self_pos, &enemy_pos, &eng_cfg);
            apply_attack_cooldown(
                &mut brain.counters,
                &mut cmd,
                entering_attack,
                eng_cfg.attack_cooldown_ticks,
            );
            continue;
        }

        // 2) Enemy 不在 / 圏外: Player follow。Chase/Attack から戻る遷移は dwell ガード対象。
        if brain.state != AllyState::Follow && can_transition {
            brain.state = AllyState::Follow;
            brain.counters.frames_since_state_entered = 0;
        }
        let Some((player_entity, player_pos)) = nearest_player else {
            // Player 不在 (= scene 起動直後やデバッグ): 何もしない。
            brain.target = None;
            *cmd = AiCommand::default();
            continue;
        };
        brain.target = Some(player_entity);
        let dist_to_player = distance_xz(self_pos, &player_pos);
        // Follow hysteresis: min 未満で停止、max 超で追従再開。間 (= dead zone) では現在の
        // 動作を維持するため「直前の cmd が move を出していたか」で判定したいが、Brain は
        // cmd の履歴を持たない。代わりに「`(min + max) / 2` を境界に dead zone 内では
        // 距離が大きい側 (> midpoint) のとき follow を継続」近似で済ます。実用上 hysteresis
        // 幅 40 px なら 1 decision で抜けるので体感差は出ない。
        let midpoint = f32::midpoint(
            brain.config.follow_distance_min_px,
            brain.config.follow_distance_max_px,
        );
        let should_follow = if dist_to_player > brain.config.follow_distance_max_px {
            true
        } else if dist_to_player < brain.config.follow_distance_min_px {
            false
        } else {
            dist_to_player > midpoint
        };
        *cmd = if should_follow {
            // Follow は「Player の隣に立つ」のが目的: X stop ratio は使わず、follow_distance_min
            // を X 軸 dead zone の代替として使う。Z は 8px dead zone を共有。
            follow_command(self_pos, &player_pos, brain.config.follow_distance_min_px)
        } else {
            AiCommand::default()
        };
    }
}

// ──────────────── BotBrain (Hero 自動化) ────────────────

/// Player 自動化 (デモプレイ / オートバトル / デバッグ bot) 用 Brain (ADR-0035 Phase 3 +
/// ADR-0039)。FSM 3 状態 ([`EngagementState`]) を持ち、`MeleeBrain` と同形のロジックで動く。
/// `MeleeBrain` との差は (1) self marker が Hero+Ai、(2) target が `Side::Villain`、
/// (3) Wander 無し (target 不在は Idle で待機) の 3 点だけ。
///
/// 競合解決 (= Player の手動入力との排他): `BotBrain` 自身は何もせず、
/// [`super::movement::player_input_controller`] の Query に `Without<BotBrain>` を入れる
/// ことで自然に手動入力 system を skip させる (ADR-0035 Phase 3 補追・案 A 排他)。
///
/// Config は ADR-0039 で `MeleeConfig` を引き続き流用 (`BotConfig::into_melee_config` で
/// 詰め替え)。Bot 専用 param (perception / panic / replay 等) が必要になったら `BotConfig`
/// に追加する。
#[derive(Component, Debug, Clone)]
pub struct BotBrain {
    pub state: EngagementState,
    pub target: Option<Entity>,
    pub counters: BrainCounters,
    pub config: MeleeConfig,
}

impl BotBrain {
    #[must_use]
    pub fn new(config: MeleeConfig) -> Self {
        Self {
            state: EngagementState::Idle,
            target: None,
            counters: BrainCounters::default(),
            config,
        }
    }
}

impl Brain for BotBrain {
    fn engagement(&self) -> &EngagementConfig {
        &self.config.engagement
    }
    fn selector(&self) -> TargetSelector {
        self.config.selector
    }
    fn counters_mut(&mut self) -> &mut BrainCounters {
        &mut self.counters
    }
    fn counters(&self) -> &BrainCounters {
        &self.counters
    }
    fn target(&self) -> Option<Entity> {
        self.target
    }
    fn set_target(&mut self, target: Option<Entity>) {
        self.target = target;
    }
}

/// `BotBrain` の tick (ADR-0035 Phase 3 / ADR-0038 / ADR-0039)。`MeleeBrain` の tick と同形:
/// decision を `decision_interval_ticks` 周期で間引きつつ、dwell + cooldown で
/// チャタリングを構造的に防ぐ。target は最も近い Villain side、不在なら Idle に
/// 戻して AiCommand を全 0 にする (= Player は止まる)。
///
/// ADR-0038: BotBrain owner = Hero + Ai。Self の Side check は不要、target query は
/// `Without<BotBrain>` で self を除外する。
fn bot_brain_tick(
    mut brains: Query<(&mut BotBrain, &WorldPosition, &mut AiCommand)>,
    villain_targets: Query<(Entity, &WorldPosition, &Side), Without<BotBrain>>,
) {
    for (mut brain, self_pos, mut cmd) in &mut brains {
        let decision_interval = brain.config.engagement.decision_interval_ticks;
        if !tick_counters(&mut brain.counters, decision_interval) {
            continue;
        }

        let selector = brain.config.selector;
        let prev_target = brain.target;
        let target = select_target(
            self_pos,
            Side::Villain,
            selector,
            prev_target,
            &villain_targets,
        );
        brain.target = target.map(|(e, _)| e);

        let Some((_, target_pos)) = target else {
            if brain.state != EngagementState::Idle {
                brain.state = EngagementState::Idle;
                brain.counters.frames_since_state_entered = 0;
            }
            *cmd = AiCommand::default();
            continue;
        };

        let dist = distance_xz(self_pos, &target_pos);
        let eng_cfg = brain.config.engagement.clone();
        let can_transition = brain.counters.frames_since_state_entered >= eng_cfg.min_dwell_ticks;
        let next_state = decide_engagement(
            brain.state,
            dist,
            can_transition,
            &eng_cfg,
            brain.counters.attack_cooldown_remaining,
        );
        let entering_attack =
            next_state == EngagementState::Attack && brain.state != EngagementState::Attack;
        if next_state != brain.state {
            brain.state = next_state;
            brain.counters.frames_since_state_entered = 0;
        }

        *cmd = engagement_command(brain.state, self_pos, &target_pos, &eng_cfg);
        apply_attack_cooldown(
            &mut brain.counters,
            &mut cmd,
            entering_attack,
            eng_cfg.attack_cooldown_ticks,
        );
    }
}

// ──────────────── Actuator ────────────────

/// `AiCommand` を `CharacterState` / `KinematicVel` / `Facing` / `WorldPosition` に焼き込む。
/// 元 `movement::handle_input` の優先度ロジック (Guard > Jump > Attack > DownAttack > 移動)
/// と `is_locked()` skip 規約をそのまま引き継ぐ。
///
/// `movement::sync_*` / `camera_follow` は `AnimationSet::Tick` のあとに走るので、本 system は
/// **入力反映 system** として `handle_input` と同じ位置 (Tick の前) で動けばよい。順序付けは
/// `MovementPlugin` 側の `.after(apply_command)` で行う。
pub fn apply_command(
    level: Option<Res<Level>>,
    mut query: Query<(
        &AiCommand,
        &mut WorldPosition,
        &mut Facing,
        &mut CharacterState,
        &mut KinematicVel,
        &PhysicsParams,
    )>,
) {
    // 60Hz 固定 (元 handle_input と同じ理由: vsync ブレを乗せたくない)。
    let dt = VSYNC_TICK_SECS;
    let step = MOVE_SPEED_PX_PER_SEC * dt;
    let contains = |x: f32, z: f32| level.as_deref().is_none_or(|l| l.contains_xz(x, z));

    for (cmd, mut pos, mut facing, mut state, mut vel, phys) in &mut query {
        let dx = cmd.move_x * step;
        let dz = cmd.move_z * step;
        let move_target_state = if cmd.move_x == 0.0 && cmd.move_z == 0.0 {
            CharacterState::Idle
        } else {
            CharacterState::Walk
        };

        // 強制向きは state 判定より先に反映 (face 指定があれば移動の符号より優先)。
        if let Some(f) = cmd.face {
            *facing = f;
        }

        // Jump 中 (非 locked): 空中移動と向き更新を許し、attack で JumpAttack へ。
        if matches!(*state, CharacterState::Jump) {
            if dx != 0.0 || dz != 0.0 {
                let next = step_axis_aware(*pos, dx, dz, contains);
                pos.x = next.x;
                pos.z = next.z;
                update_facing_from_move(cmd.move_x, &mut facing);
            }
            if cmd.attack {
                *state = CharacterState::JumpAttack;
            }
            continue;
        }
        if state.is_locked() {
            continue;
        }
        // Guard 中 (非 locked): guard 押下続いていれば維持、離せば Idle。
        if matches!(*state, CharacterState::Guard) {
            if !cmd.guard {
                *state = CharacterState::Idle;
            }
            continue;
        }
        // 地上の通常入力。優先度: Guard > Jump > Attack > DownAttack > 移動。
        // Guard と Jump は地上 (pos.y == 0) のみ受付 (ADR-0027/0028)。
        if cmd.guard && pos.y == 0.0 {
            *state = CharacterState::Guard;
            continue;
        }
        if cmd.jump && pos.y == 0.0 {
            #[allow(clippy::cast_possible_truncation)]
            let jv = phys.0.jump_velocity_y as f32;
            vel.vel_y = jv;
            *state = CharacterState::Jump;
            continue;
        }
        if cmd.attack {
            *state = CharacterState::Attack;
            continue;
        }
        if cmd.down_attack {
            *state = CharacterState::DownAttack;
            continue;
        }
        if dx != 0.0 || dz != 0.0 {
            let next = step_axis_aware(*pos, dx, dz, contains);
            pos.x = next.x;
            pos.z = next.z;
            update_facing_from_move(cmd.move_x, &mut facing);
        }
        // Changed<> をぶれずに発火させるため等価チェックして必要なときだけ書く。
        if *state != move_target_state {
            *state = move_target_state;
        }
    }
}

/// `move_x` の符号で `Facing` を更新。`|move_x| < FACING_DEAD_ZONE` では維持 (= 微小振動による
/// 左右ピクピクを防ぐ、ADR-0035 チャタリング防止 軸 3)。元 `handle_input` の「dx == 0 なら
/// facing 維持」規約は dead zone (0.15) の degenerate case として包含される。
fn update_facing_from_move(move_x: f32, facing: &mut Facing) {
    if move_x.abs() >= FACING_DEAD_ZONE {
        *facing = if move_x > 0.0 {
            Facing::Right
        } else {
            Facing::Left
        };
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // bit-exact 比較 (Default / 定数の直接代入)
mod tests {
    use super::*;

    #[test]
    fn ai_command_default_is_neutral() {
        let cmd = AiCommand::default();
        assert_eq!(cmd.move_x, 0.0);
        assert_eq!(cmd.move_z, 0.0);
        assert!(!cmd.attack);
        assert!(!cmd.down_attack);
        assert!(!cmd.jump);
        assert!(!cmd.guard);
        assert!(cmd.face.is_none());
    }

    #[test]
    fn facing_update_respects_dead_zone() {
        let mut facing = Facing::Right;
        // dead zone (|0.1| < 0.15) では維持
        update_facing_from_move(0.1, &mut facing);
        assert_eq!(facing, Facing::Right);
        update_facing_from_move(-0.1, &mut facing);
        assert_eq!(facing, Facing::Right);
        // dead zone 外 (|0.2| >= 0.15) では更新
        update_facing_from_move(-0.2, &mut facing);
        assert_eq!(facing, Facing::Left);
        update_facing_from_move(0.2, &mut facing);
        assert_eq!(facing, Facing::Right);
    }

    #[test]
    fn facing_update_at_zero_keeps_current() {
        // move_x == 0 は dead zone 内 (handle_input の「dx == 0 なら facing 維持」と一致)。
        let mut facing = Facing::Left;
        update_facing_from_move(0.0, &mut facing);
        assert_eq!(facing, Facing::Left);
    }

    #[test]
    fn facing_update_at_unit_input_flips_correctly() {
        // Player の PlayerInputController は ±1.0 / 0 のデジタル値を出すので、これが
        // dead zone を必ず越えることを確認 (= 既存 Player 挙動の互換)。
        let mut facing = Facing::Right;
        update_facing_from_move(-1.0, &mut facing);
        assert_eq!(facing, Facing::Left);
        update_facing_from_move(1.0, &mut facing);
        assert_eq!(facing, Facing::Right);
    }

    fn eng_cfg() -> EngagementConfig {
        EngagementConfig::default()
    }

    #[test]
    fn engagement_idle_stays_when_target_out_of_chase_range() {
        let c = eng_cfg();
        // chase_enter 200 を超える距離なら Idle 維持。dwell は無関係。
        assert_eq!(
            decide_engagement(EngagementState::Idle, 250.0, true, &c, 0),
            EngagementState::Idle,
        );
    }

    #[test]
    fn engagement_idle_enters_chase_when_target_within_chase_range() {
        let c = eng_cfg();
        // 150 px (chase_enter 200 内、attack_enter 28 外) → Chase。
        // Idle からの初動は dwell ガードを掛けない (起動時に即動けるよう)。
        assert_eq!(
            decide_engagement(EngagementState::Idle, 150.0, false, &c, 0),
            EngagementState::Chase,
        );
    }

    #[test]
    fn engagement_idle_jumps_to_attack_when_already_in_attack_range() {
        let c = eng_cfg();
        // 距離 0 (= 重なり) + cooldown 0 → Idle → Attack に直行できる (Chase スキップ)。
        assert_eq!(
            decide_engagement(EngagementState::Idle, 10.0, true, &c, 0),
            EngagementState::Attack,
        );
    }

    #[test]
    fn engagement_chase_exits_to_idle_only_past_chase_exit_threshold() {
        let c = eng_cfg();
        // chase_enter 200 を越えても chase_exit 240 内なら Chase 維持 (hysteresis 軸 1)。
        assert_eq!(
            decide_engagement(EngagementState::Chase, 220.0, true, &c, 0),
            EngagementState::Chase,
        );
        // chase_exit を越えたら Idle。
        assert_eq!(
            decide_engagement(EngagementState::Chase, 250.0, true, &c, 0),
            EngagementState::Idle,
        );
    }

    #[test]
    fn engagement_chase_enters_attack_when_close_and_cooldown_clear() {
        let c = eng_cfg();
        // attack_enter 28 内 + cooldown 0 → Attack。
        assert_eq!(
            decide_engagement(EngagementState::Chase, 20.0, true, &c, 0),
            EngagementState::Attack,
        );
        // cooldown 残存 → Attack は出さず Chase 維持 (距離を詰めて待つ)。
        assert_eq!(
            decide_engagement(EngagementState::Chase, 20.0, true, &c, 30),
            EngagementState::Chase,
        );
    }

    #[test]
    fn engagement_chase_dwell_blocks_transition() {
        let c = eng_cfg();
        // can_transition=false (= dwell 未経過) は距離が遠くても Chase 維持。
        assert_eq!(
            decide_engagement(EngagementState::Chase, 500.0, false, &c, 0),
            EngagementState::Chase,
        );
        assert_eq!(
            decide_engagement(EngagementState::Chase, 10.0, false, &c, 0),
            EngagementState::Chase,
        );
    }

    #[test]
    fn engagement_attack_falls_back_to_chase_after_cooldown_or_past_exit() {
        let c = eng_cfg();
        // cooldown 残存 → 必ず Chase に戻る (= 攻撃しっぱなしを防ぐ)。
        assert_eq!(
            decide_engagement(EngagementState::Attack, 10.0, true, &c, 30),
            EngagementState::Chase,
        );
        // attack_exit 36 を越えたら Chase。
        assert_eq!(
            decide_engagement(EngagementState::Attack, 40.0, true, &c, 0),
            EngagementState::Chase,
        );
        // 攻撃範囲内 + cooldown 0 → Attack 維持。
        assert_eq!(
            decide_engagement(EngagementState::Attack, 30.0, true, &c, 0),
            EngagementState::Attack,
        );
    }

    #[test]
    fn engagement_attack_dwell_blocks_immediate_exit() {
        let c = eng_cfg();
        // dwell 未経過 (frames_since_state_entered < min_dwell_ticks) は cooldown / 距離無関係に
        // Attack 維持。「Attack に入った瞬間に Chase に弾き返す」のを防ぐ (1 frame で再起動を防止)。
        assert_eq!(
            decide_engagement(EngagementState::Attack, 200.0, false, &c, 60),
            EngagementState::Attack,
        );
    }

    #[test]
    fn engagement_command_chase_emits_unit_vector_toward_target_outside_dead_zone() {
        // self=(0,0,0)、target=(300,_,400) → 十分遠いので X/Z dead zone は効かず単位 (0.6, 0.8)。
        let self_pos = WorldPosition::new(0.0, 0.0, 0.0);
        let target = WorldPosition::new(300.0, 0.0, 400.0);
        let cmd = engagement_command(EngagementState::Chase, &self_pos, &target, &eng_cfg());
        assert!((cmd.move_x - 0.6).abs() < 1e-6);
        assert!((cmd.move_z - 0.8).abs() < 1e-6);
        assert!(!cmd.attack);
    }

    #[test]
    fn engagement_command_attack_emits_attack_desire_only() {
        let self_pos = WorldPosition::new(0.0, 0.0, 0.0);
        let target = WorldPosition::new(20.0, 0.0, 0.0);
        let cmd = engagement_command(EngagementState::Attack, &self_pos, &target, &eng_cfg());
        assert!(cmd.attack);
        assert_eq!(cmd.move_x, 0.0);
        assert_eq!(cmd.move_z, 0.0);
    }

    #[test]
    fn engagement_command_chase_when_overlapping_emits_zero() {
        // self == target なら割り算 0 を回避して全 0 を返す (NaN を AiCommand に流さない)。
        let p = WorldPosition::new(10.0, 0.0, 20.0);
        let cmd = engagement_command(EngagementState::Chase, &p, &p, &eng_cfg());
        assert_eq!(cmd.move_x, 0.0);
        assert_eq!(cmd.move_z, 0.0);
    }

    #[test]
    fn engagement_command_chase_stops_x_inside_attack_range_dead_zone() {
        // attack_enter_range_px = 28、X stop ratio 0.85 → X 23.8 以下では move_x = 0。
        // Z は dead zone (2px) を越えているので Z 方向だけ動く。
        let self_pos = WorldPosition::new(0.0, 0.0, 0.0);
        let target = WorldPosition::new(20.0, 0.0, 50.0);
        let cmd = engagement_command(EngagementState::Chase, &self_pos, &target, &eng_cfg());
        assert_eq!(cmd.move_x, 0.0, "X dead zone 内は動かない (重なり防止)");
        assert!(cmd.move_z > 0.0, "Z 方向は dead zone 外なので進む");
    }

    #[test]
    fn engagement_command_chase_stops_both_axes_when_in_attack_box_position() {
        // X も Z も dead zone 内 → 完全停止 (= Attack 範囲に既に居る、Brain は次 frame で Attack 遷移)。
        let self_pos = WorldPosition::new(0.0, 0.0, 0.0);
        let target = WorldPosition::new(20.0, 0.0, 1.0);
        let cmd = engagement_command(EngagementState::Chase, &self_pos, &target, &eng_cfg());
        assert_eq!(cmd.move_x, 0.0);
        assert_eq!(cmd.move_z, 0.0);
    }

    #[test]
    fn engagement_command_chase_z_dead_zone_boundary() {
        // X は dead zone 内 (20px、X stop = 28*0.85 = 23.8) で固定し、Z 軸の挙動だけを観察する。
        let self_pos = WorldPosition::new(0.0, 0.0, 0.0);
        // Z = 7 < 8 (dead zone) → 停止
        let cmd_in = engagement_command(
            EngagementState::Chase,
            &self_pos,
            &WorldPosition::new(20.0, 0.0, 7.0),
            &eng_cfg(),
        );
        assert_eq!(cmd_in.move_z, 0.0);
        // Z = 9 > 8 → 動く
        let cmd_out = engagement_command(
            EngagementState::Chase,
            &self_pos,
            &WorldPosition::new(20.0, 0.0, 9.0),
            &eng_cfg(),
        );
        assert!(cmd_out.move_z > 0.0);
    }

    // ──────────────── AllyBrain (ADR-0035 Phase 2) ────────────────

    fn ally_cfg() -> AllyConfig {
        AllyConfig::default()
    }

    /// AllyState::Follow + attack 圏内 + cooldown 0 → AllyState::Attack に直行
    /// (`decide_engagement(Idle, ...)` の Attack 直行ルートを Ally でも検証)。
    #[test]
    fn ally_engagement_follow_to_attack_when_in_range_and_cooldown_zero() {
        let c = ally_cfg();
        let eng = c.engagement;
        let next = decide_engagement(AllyState::Follow.to_engagement(), 10.0, true, &eng, 0);
        assert_eq!(AllyState::from_engagement(next), AllyState::Attack);
    }

    /// AllyState::Follow + chase 圏内 + attack 圏外 → AllyState::Chase。
    #[test]
    fn ally_engagement_follow_to_chase_when_in_range_but_not_attack_close() {
        let c = ally_cfg();
        let eng = c.engagement;
        let next = decide_engagement(AllyState::Follow.to_engagement(), 50.0, true, &eng, 0);
        assert_eq!(AllyState::from_engagement(next), AllyState::Chase);
    }

    /// AllyState::Chase + attack 圏内 + cooldown 残存 → AllyState::Chase (cooldown 中は
    /// Attack に行かず Chase 維持)。
    #[test]
    fn ally_engagement_chase_enters_attack_only_when_cooldown_clear() {
        let c = ally_cfg();
        let eng = c.engagement;
        let next = decide_engagement(AllyState::Chase.to_engagement(), 20.0, true, &eng, 0);
        assert_eq!(AllyState::from_engagement(next), AllyState::Attack);
        let next = decide_engagement(AllyState::Chase.to_engagement(), 20.0, true, &eng, 30);
        assert_eq!(AllyState::from_engagement(next), AllyState::Chase);
    }

    /// AllyState::Attack + dwell 未経過 → AllyState::Attack 維持 (距離無関係)。
    #[test]
    fn ally_engagement_attack_dwell_blocks_immediate_exit() {
        let c = ally_cfg();
        let eng = c.engagement;
        let next = decide_engagement(AllyState::Attack.to_engagement(), 200.0, false, &eng, 60);
        assert_eq!(AllyState::from_engagement(next), AllyState::Attack);
    }

    /// AllyState::Attack + cooldown 残存 or attack_exit 超え → Chase に戻る。
    #[test]
    fn ally_engagement_attack_falls_back_to_chase_after_cooldown_or_past_exit() {
        let c = ally_cfg();
        let eng = c.engagement;
        let next = decide_engagement(AllyState::Attack.to_engagement(), 10.0, true, &eng, 30);
        assert_eq!(AllyState::from_engagement(next), AllyState::Chase);
        let next = decide_engagement(AllyState::Attack.to_engagement(), 40.0, true, &eng, 0);
        assert_eq!(AllyState::from_engagement(next), AllyState::Chase);
        let next = decide_engagement(AllyState::Attack.to_engagement(), 30.0, true, &eng, 0);
        assert_eq!(AllyState::from_engagement(next), AllyState::Attack);
    }

    #[test]
    fn follow_command_stops_inside_min_distance_and_z_dead_zone() {
        // Player との X 距離が follow_distance_min (= 40) 未満かつ Z dead zone 内 → 全 0。
        let cfg = ally_cfg();
        let self_pos = WorldPosition::new(0.0, 0.0, 0.0);
        let target = WorldPosition::new(20.0, 0.0, 4.0);
        let cmd = follow_command(&self_pos, &target, cfg.follow_distance_min_px);
        assert_eq!(cmd.move_x, 0.0);
        assert_eq!(cmd.move_z, 0.0);
    }

    #[test]
    fn follow_command_moves_toward_player_when_outside_min_distance() {
        // X 距離 60 > follow_distance_min 40 → X 方向に動く。Z は dead zone 内 → 0。
        let cfg = ally_cfg();
        let self_pos = WorldPosition::new(0.0, 0.0, 0.0);
        let target = WorldPosition::new(60.0, 0.0, 4.0);
        let cmd = follow_command(&self_pos, &target, cfg.follow_distance_min_px);
        assert!(cmd.move_x > 0.0);
        assert_eq!(cmd.move_z, 0.0);
    }

    /// 回帰テスト: Attack 進入 frame で `AiCommand.attack` が true で出ること。
    /// 過去バグ: cooldown を「state 遷移ブロック内」で仕込んだため、進入 frame の `attack: true`
    /// が直後の `cooldown > 0` 分岐で false に塗られ、apply_command まで届かなかった。
    /// 次の decision (interval 経過後) では cooldown 残存 → `attack: false` (= 抑止) を確認する。
    #[test]
    fn melee_brain_emits_attack_on_first_entry_then_suppresses_during_cooldown() {
        use crate::shared::PlayerId;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::ecs::world::World;

        let mut world = World::new();
        // ADR-0038: Hero side の Player を距離 0 に置き、Villain Enemy を attack_enter_range_px
        // 内に入れる。Hero target は MeleeBrain の hero_targets query (Without<MeleeBrain>) に
        // 入る。
        world.spawn((
            Side::Hero,
            Controller::Human,
            PlayerId::P1,
            WorldPosition::new(0.0, 0.0, 0.0),
        ));
        let enemy = world
            .spawn((
                Side::Villain,
                Controller::Ai,
                MeleeBrain::new(MeleeConfig::default()),
                WorldPosition::new(10.0, 0.0, 0.0),
                AiCommand::default(),
            ))
            .id();

        // 1 回目の tick = 進入 decision。Idle → Attack に直行する想定。
        world
            .run_system_once(melee_brain_tick)
            .expect("run_system_once: melee_brain_tick");
        let cmd = world.entity(enemy).get::<AiCommand>().expect("AiCommand");
        assert!(
            cmd.attack,
            "Attack 進入 frame は attack:true を出さなければ apply_command で発火しない",
        );
        let brain = world.entity(enemy).get::<MeleeBrain>().expect("MeleeBrain");
        assert_eq!(brain.state, EngagementState::Attack);
        assert!(brain.counters.attack_cooldown_remaining > 0);

        // decision_interval (= 6 frame) 経過させて次の decision を回す。cooldown 中なので
        // Brain は Chase に戻り、AiCommand.attack は false。
        for _ in 0..7 {
            world
                .run_system_once(melee_brain_tick)
                .expect("run_system_once: melee_brain_tick");
        }
        let cmd = world.entity(enemy).get::<AiCommand>().expect("AiCommand");
        assert!(!cmd.attack, "cooldown 中は attack:false で抑止されるべき");
    }

    // ──────────────── BotBrain (ADR-0035 Phase 3) ────────────────

    /// 回帰テスト: Attack 進入 frame で `AiCommand.attack` が true で出ること。
    /// `melee_brain_emits_attack_on_first_entry_...` の Bot 版 (Player + BotBrain に
    /// Enemy を attack_enter_range_px 内に置く)。
    #[test]
    fn bot_emits_attack_on_first_entry_then_suppresses_during_cooldown() {
        use crate::shared::PlayerId;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::ecs::world::World;

        let mut world = World::new();
        // ADR-0038: BotBrain owner は Hero + Ai (= Player 自動化)、target は Villain side。
        // BotBrain attach されているので Controller は Ai 扱い (PlayerInputController が
        // skip する)。PlayerId は HUD 引きの都合で残す。
        let player = world
            .spawn((
                Side::Hero,
                Controller::Ai,
                PlayerId::P1,
                BotBrain::new(MeleeConfig::default()),
                WorldPosition::new(0.0, 0.0, 0.0),
                AiCommand::default(),
            ))
            .id();
        world.spawn((
            Side::Villain,
            Controller::Ai,
            WorldPosition::new(10.0, 0.0, 0.0),
        ));

        world
            .run_system_once(bot_brain_tick)
            .expect("run_system_once: bot_brain_tick");
        let cmd = world.entity(player).get::<AiCommand>().expect("AiCommand");
        assert!(
            cmd.attack,
            "Attack 進入 frame は attack:true を出さなければ apply_command で発火しない",
        );
        let brain = world.entity(player).get::<BotBrain>().expect("BotBrain");
        assert_eq!(brain.state, EngagementState::Attack);
        assert!(brain.counters.attack_cooldown_remaining > 0);

        for _ in 0..7 {
            world
                .run_system_once(bot_brain_tick)
                .expect("run_system_once: bot_brain_tick");
        }
        let cmd = world.entity(player).get::<AiCommand>().expect("AiCommand");
        assert!(!cmd.attack, "cooldown 中は attack:false で抑止されるべき");
    }

    /// Enemy 不在で BotBrain は Idle に戻し AiCommand を全 0 にする。Phase 2 の
    /// AllyBrain と違って Player follow への fallback は無い (= Wander 無し設計)。
    #[test]
    fn bot_resets_to_idle_when_enemies_disappear() {
        use crate::shared::PlayerId;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::ecs::world::World;

        let mut world = World::new();
        let mut brain = BotBrain::new(MeleeConfig::default());
        // 「以前 Chase だった」想定の state を仕込む (Villain 不在 tick で Idle に戻すことを
        // 確認)。
        brain.state = EngagementState::Chase;
        brain.counters.frames_since_state_entered = 100;
        // ADR-0038: BotBrain は Hero + Ai に attach。
        let player = world
            .spawn((
                Side::Hero,
                Controller::Ai,
                PlayerId::P1,
                brain,
                WorldPosition::new(0.0, 0.0, 0.0),
                AiCommand {
                    move_x: 1.0,
                    ..AiCommand::default()
                },
            ))
            .id();

        world
            .run_system_once(bot_brain_tick)
            .expect("run_system_once: bot_brain_tick");

        let brain = world.entity(player).get::<BotBrain>().expect("BotBrain");
        assert_eq!(brain.state, EngagementState::Idle);
        assert!(brain.target.is_none());
        let cmd = world.entity(player).get::<AiCommand>().expect("AiCommand");
        assert_eq!(cmd.move_x, 0.0);
        assert_eq!(cmd.move_z, 0.0);
        assert!(!cmd.attack);
    }

    // ──────────────── TargetSelector (ADR-0039) ────────────────

    /// `TargetSelector::LastEngaged` で MeleeBrain が初回 tick で nearest を選び、それ以降
    /// 「より近い候補」が出現しても **前回 target を継続追跡** することを確認する
    /// (Phase 2 補追動機の最小回帰)。
    #[test]
    fn select_target_last_engaged_keeps_previous_target_even_when_a_closer_one_appears() {
        use crate::shared::PlayerId;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::ecs::world::World;

        let mut world = World::new();
        // ADR-0038: MeleeBrain は Villain side。target は Hero side。
        // chase 圏内 (100 px) に Hero A を置き、tick 後により近い Hero B (5 px) を追加しても
        // brain.target が A のままであることを確認する (= LastEngaged 継続追跡)。
        let hero_a = world
            .spawn((
                Side::Hero,
                Controller::Human,
                PlayerId::P1,
                WorldPosition::new(100.0, 0.0, 0.0),
            ))
            .id();
        let enemy = world
            .spawn((
                Side::Villain,
                Controller::Ai,
                MeleeBrain::new(MeleeConfig {
                    selector: TargetSelector::LastEngaged,
                    ..MeleeConfig::default()
                }),
                WorldPosition::new(0.0, 0.0, 0.0),
                AiCommand::default(),
            ))
            .id();

        world.run_system_once(melee_brain_tick).expect("first tick");
        let brain = world.entity(enemy).get::<MeleeBrain>().expect("MeleeBrain");
        assert_eq!(
            brain.target,
            Some(hero_a),
            "初回 tick は nearest fallback で A を選ぶ",
        );

        // より近い Hero B を追加。LastEngaged は前回 target を維持する。
        world.spawn((
            Side::Hero,
            Controller::Human,
            WorldPosition::new(5.0, 0.0, 0.0),
        ));

        // decision_interval (= 6) を超えるよう 7 frame 走らせて 2 回目の decision を回す。
        for _ in 0..7 {
            world
                .run_system_once(melee_brain_tick)
                .expect("subsequent tick");
        }
        let brain = world.entity(enemy).get::<MeleeBrain>().expect("MeleeBrain");
        assert_eq!(
            brain.target,
            Some(hero_a),
            "LastEngaged は前回 target (= A) を継続追跡するべき",
        );
    }

    /// `TargetSelector::LastEngaged` で前回 target が despawn されたら Nearest にフォールバック。
    #[test]
    fn select_target_last_engaged_falls_back_to_nearest_when_target_disappears() {
        use crate::shared::PlayerId;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::ecs::world::World;

        let mut world = World::new();
        let hero_a = world
            .spawn((
                Side::Hero,
                Controller::Human,
                PlayerId::P1,
                WorldPosition::new(100.0, 0.0, 0.0),
            ))
            .id();
        let hero_b = world
            .spawn((
                Side::Hero,
                Controller::Human,
                WorldPosition::new(150.0, 0.0, 0.0),
            ))
            .id();
        let enemy = world
            .spawn((
                Side::Villain,
                Controller::Ai,
                MeleeBrain::new(MeleeConfig {
                    selector: TargetSelector::LastEngaged,
                    ..MeleeConfig::default()
                }),
                WorldPosition::new(0.0, 0.0, 0.0),
                AiCommand::default(),
            ))
            .id();

        world.run_system_once(melee_brain_tick).expect("first tick");
        let brain = world.entity(enemy).get::<MeleeBrain>().expect("MeleeBrain");
        assert_eq!(brain.target, Some(hero_a));

        // hero_a を despawn。次の decision で hero_b に切り替わる。
        world.entity_mut(hero_a).despawn();
        for _ in 0..7 {
            world
                .run_system_once(melee_brain_tick)
                .expect("subsequent tick");
        }
        let brain = world.entity(enemy).get::<MeleeBrain>().expect("MeleeBrain");
        assert_eq!(
            brain.target,
            Some(hero_b),
            "前回 target が消えたら Nearest fallback で hero_b に切り替わる",
        );
    }

    /// `TargetSelector::Random` / `WeightedByThreat` は stub。warn + Nearest fallback で
    /// MeleeBrain が動くことを担保する (= YAML に書いても破綻しない最小保証)。
    #[test]
    fn select_target_stub_variants_fall_back_to_nearest() {
        use crate::shared::PlayerId;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::ecs::world::World;

        for stub in [TargetSelector::Random, TargetSelector::WeightedByThreat] {
            let mut world = World::new();
            let hero = world
                .spawn((
                    Side::Hero,
                    Controller::Human,
                    PlayerId::P1,
                    WorldPosition::new(50.0, 0.0, 0.0),
                ))
                .id();
            let enemy = world
                .spawn((
                    Side::Villain,
                    Controller::Ai,
                    MeleeBrain::new(MeleeConfig {
                        selector: stub,
                        ..MeleeConfig::default()
                    }),
                    WorldPosition::new(0.0, 0.0, 0.0),
                    AiCommand::default(),
                ))
                .id();

            world
                .run_system_once(melee_brain_tick)
                .expect("tick on stub selector");
            let brain = world.entity(enemy).get::<MeleeBrain>().expect("MeleeBrain");
            assert_eq!(
                brain.target,
                Some(hero),
                "{stub:?} stub は Nearest fallback で hero を target にする",
            );
        }
    }

    /// ADR-0038: MeleeBrain (Villain) は target 候補として `Side::Hero` 全体 (Player + Ally)
    /// を見る。Player と Ally の両方を world に置き、近い方が選ばれることを確認する
    /// (= Phase 4 の主要動機: Enemy → Ally chase 非対称の解消の最小テスト)。
    #[test]
    fn melee_brain_targets_nearest_hero_regardless_of_controller() {
        use crate::shared::PlayerId;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::ecs::world::World;

        let mut world = World::new();
        // Player (Hero+Human) は離れた位置に、Ally (Hero+Ai) を Enemy のすぐ隣に置く。
        // Ally が近いので Enemy はそちらを target にするはず。
        world.spawn((
            Side::Hero,
            Controller::Human,
            PlayerId::P1,
            WorldPosition::new(500.0, 0.0, 0.0),
        ));
        let ally = world
            .spawn((
                Side::Hero,
                Controller::Ai,
                AllyBrain::new(AllyConfig::default()),
                WorldPosition::new(40.0, 0.0, 0.0),
                AiCommand::default(),
            ))
            .id();
        let enemy = world
            .spawn((
                Side::Villain,
                Controller::Ai,
                MeleeBrain::new(MeleeConfig::default()),
                WorldPosition::new(20.0, 0.0, 0.0),
                AiCommand::default(),
            ))
            .id();

        world
            .run_system_once(melee_brain_tick)
            .expect("run_system_once: melee_brain_tick");

        let brain = world.entity(enemy).get::<MeleeBrain>().expect("MeleeBrain");
        assert_eq!(
            brain.target,
            Some(ally),
            "Enemy は最も近い Hero (= Ally) を target にするべき",
        );
    }
}
