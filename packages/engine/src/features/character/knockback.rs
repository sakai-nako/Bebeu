//! Knockback / 吹っ飛びフロー (ADR-0024 / Phase A + B) — FSD: feature slice。
//!
//! Phase A + B でカバーする範囲:
//! - [`Combatant`] が gauge / 回復 timer / stage timer / 残バウンド数 / [`FinalAction`]
//!   を保持する。
//! - [`KinematicVel`] が吹っ飛び中の victim 速度を保持し、`apply_velocity` が
//!   `WorldPosition` に dt 積分する (dt = 60Hz 固定)。
//! - [`PhysicsParams`] は `Character.physics` を entity に持たせる wrapper。
//! - 物理ステージ遷移:
//!     KnockbackUp →(apex)→ KnockbackDown →(着地)→
//!       remaining_bounces>0: BounceUp →(apex)→ BounceDown →(着地)→ (再度判定)
//!       remaining_bounces=0: Slide →(摩擦で停止)→ LieDown →
//!         FinalAction=LieDown: →(timer)→ Rise →(timer)→ Idle (gauge / bounce / FinalAction reset)
//!         FinalAction=Dead:    永続停止 (= KO 演出、ADR-0025)
//!
//! Phase C 以降の予定:
//! - HitFromBehind 判定と Animation 4 段フォールバック (ADR-0025)
//!
//! 全 system が `HitStopState` 中の entity を skip する (= hit_stop で完全 freeze)。
use bevy::prelude::*;

use crate::entities::character::Physics;

use super::animation::{AnimationFrames, AnimationSet, VSYNC_TICK_SECS};
use super::debug_control::SimulationSet;
use super::hit_stop::HitStopState;
use super::movement::WorldPosition;
use super::state_machine::CharacterState;

/// 吹っ飛びフロー終端の挙動を決めるフラグ (ADR-0025)。
/// - `LieDown` (default): `LieDown` から `Rise` に進んで Idle に戻る。
/// - `Dead`: `LieDown` から進まず、永続停止 (= KO 演出)。Animation 終端でも timer 終了でも
///   遷移を行わない。
///
/// 吹っ飛び発動時に attack 解決側が「致命傷だったか」を見て `Dead` を立てる。
/// 1 回の吹っ飛びフロー全体を通じて保持し、次の Idle 復帰 (`Rise → Idle`) で初期化する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FinalAction {
    #[default]
    LieDown,
    Dead,
}

/// 被弾耐性 (Knockback) ゲージと、吹っ飛びフロー全体で使う runtime カウンタ。
/// Player / Enemy 両方に attach する。
#[derive(Component, Debug, Clone)]
pub struct Combatant {
    /// 現在のゲージ値。閾値で初期化し、`AttackBoxMeta.knockback_damage` で削られる。
    /// 0 以下で吹っ飛び発動 (= 削りきった瞬間)。i32 で under-flow を表現する。
    pub gauge: i32,
    /// 直近の Hit から続いている回復までの残り tick。0 で「回復中ではない」。
    /// `Physics.hit_recovery_ms` を tick 換算した値を Hit 時に充填する。
    pub gauge_recovery_remaining_ticks: u32,
    /// `LieDown` / `Rise` の固定 timer (60Hz tick)。0 で「timer 切れ or 未充填」。
    /// `advance_stage_timer` が消化し、anim 終端と OR 条件で次段に進める (ADR-0025
    /// 二重終了条件)。
    pub stage_timer_ticks: u32,
    /// 残りバウンド回数。`Physics.bounce_count` で初期化し、`KnockbackDown` /
    /// `BounceDown` 着地ごとに 1 消費する。0 で次の着地は `Slide` に倒れる。
    /// 吹っ飛び発動時に attack 解決側が `bounce_count` で reset する。
    pub remaining_bounces: u32,
    /// 吹っ飛びフロー終端の挙動 (ADR-0025)。default `LieDown` で Rise → Idle、`Dead`
    /// で LieDown 永続停止。
    pub final_action: FinalAction,
    /// 背後被弾か (ADR-0025)。`true` のとき Animation 解決層が `back_*` / `dead_back_*`
    /// 系を優先する。吹っ飛び発動時に attack 解決側が attacker の Facing と effective
    /// knockback ベクトルから判定して立てる。1 回の吹っ飛びフロー全体を通じて保持し、
    /// 次の Idle 復帰 (`Rise → Idle`) で false にリセット。
    pub hit_from_behind: bool,
    /// 1 連続コンボあたりの「空中再被弾 → knockback 再発火」回数。`Physics.max_juggle_count`
    /// を超えた airborne hit は **完全無敵** (damage / state / gauge / consumed 全て不発、
    /// AABB ヒットしても素通り) になる (= 永久パターン回避)。
    /// Rise → Idle で 0 に reset (= コンボ終了で freshness 復活)。
    pub juggle_count: u32,
    /// 1 連続コンボあたりの「DownHit 遷移」回数。`Physics.max_down_hit_count` を超えた down
    /// hit は **完全無敵** (damage / state / gauge / consumed 全て不発、AABB ヒットしても
    /// 素通り) になる (= 倒れたまま無敵)。
    /// Rise → Idle で 0 に reset。
    pub down_hit_count: u32,
    /// Guard 中の被弾で削られる guard ゲージ (ADR-0028)。`Physics.guard_break_threshold` で
    /// 初期化し、`AttackBoxMeta.guard_damage` で削る。0 以下で GuardBreak 発動 (= KnockbackUp
    /// に遷移して既存物理に合流)。
    pub guard_gauge: i32,
    /// 直近のガード被弾から続いている回復までの残り tick。0 で「回復中ではない」。
    /// `Physics.guard_recovery_ms` を tick 換算した値をガード被弾時に充填する。
    pub guard_recovery_remaining_ticks: u32,
}

impl Combatant {
    /// gauge を `knockback_threshold`、remaining_bounces を `bounce_count` で初期化する。
    /// final_action / hit_from_behind は default (LieDown / false) で開始。
    #[must_use]
    pub fn new(physics: &Physics) -> Self {
        Self {
            gauge: i32::try_from(physics.knockback_threshold).unwrap_or(i32::MAX),
            gauge_recovery_remaining_ticks: 0,
            stage_timer_ticks: 0,
            remaining_bounces: physics.bounce_count,
            final_action: FinalAction::default(),
            hit_from_behind: false,
            juggle_count: 0,
            down_hit_count: 0,
            guard_gauge: i32::try_from(physics.guard_break_threshold).unwrap_or(i32::MAX),
            guard_recovery_remaining_ticks: 0,
        }
    }
}

/// 吹っ飛び中の victim が保持する速度ベクトル (画像 pixel / 秒)。
/// 攻撃ヒットで [`AttackBoxMeta.knockback`] を Facing で符号反転して充填する。
#[derive(Component, Debug, Clone, Copy, Default)]
#[allow(clippy::struct_field_names)] // vel_x/y/z は ADR-0017 の軸名と揃えるため意図的
pub struct KinematicVel {
    pub vel_x: f32,
    pub vel_y: f32,
    pub vel_z: f32,
}

/// `Character.physics` をそのまま entity に持たせる wrapper。
/// gravity / hit_recovery_ms / lie_down_duration_ms 等の per-character 値を引く。
#[derive(Component, Debug, Clone)]
pub struct PhysicsParams(pub Physics);

pub struct KnockbackPlugin;

impl Plugin for KnockbackPlugin {
    fn build(&self, app: &mut App) {
        // 全部 .after(AnimationSet::Tick) に乗せて、anim.is_finished() を「今 frame の
        // tick 後の状態」で読む。`chain()` で順序を固定して、同 frame で「重力 / 摩擦 →
        // 速度積分 → 頂点 / 着地検知 → stage 遷移」が一気通貫に走るようにする。
        // apply_slide_friction は apply_velocity の前に置いて semi-implicit に整える。
        // recover_gauge は他系統と独立なので順序は問わないが chain に同居させて
        // ScheduleOrder の指定箇所を減らす。
        app.add_systems(
            Update,
            (
                recover_gauge,
                recover_guard_gauge,
                apply_gravity,
                apply_slide_friction,
                apply_velocity,
                transition_at_apex,
                detect_landing,
                advance_guard_break,
                advance_stage_timer,
            )
                .chain()
                .after(AnimationSet::Tick)
                .in_set(SimulationSet::Active),
        );
    }
}

/// `ms` を 60Hz tick 数に変換する (ceil)。`ms = 0` で 0 tick、`1` 以上で 1 tick 以上を返す
/// ことで「YAML で書いた小さい ms が 0 tick に丸まって即遷移」を防ぐ。
#[must_use]
pub fn ms_to_ticks(ms: u32) -> u32 {
    u32::try_from(u64::from(ms).saturating_mul(60).div_ceil(1000)).unwrap_or(u32::MAX)
}

/// Hit から `hit_recovery_ms` 経過したら gauge を `knockback_threshold` まで戻す。
/// `gauge_recovery_remaining_ticks` が `> 0` のときだけ毎 tick 減らし、0 になった瞬間に
/// gauge を full 回復する。連続 Hit で `gauge_recovery_remaining_ticks` が refresh される
/// 想定 (= 短時間の連打中は回復しない、間隔があくと一気に戻る)。
fn recover_gauge(mut q: Query<(&mut Combatant, &PhysicsParams), Without<HitStopState>>) {
    for (mut combatant, phys) in &mut q {
        if combatant.gauge_recovery_remaining_ticks == 0 {
            continue;
        }
        combatant.gauge_recovery_remaining_ticks -= 1;
        if combatant.gauge_recovery_remaining_ticks == 0 {
            combatant.gauge = i32::try_from(phys.0.knockback_threshold).unwrap_or(i32::MAX);
        }
    }
}

/// ADR-0028: ガード被弾から `guard_recovery_ms` 経過したら `guard_gauge` を
/// `guard_break_threshold` まで戻す。`recover_gauge` と同型の自然回復モデル。
fn recover_guard_gauge(mut q: Query<(&mut Combatant, &PhysicsParams), Without<HitStopState>>) {
    for (mut combatant, phys) in &mut q {
        if combatant.guard_recovery_remaining_ticks == 0 {
            continue;
        }
        combatant.guard_recovery_remaining_ticks -= 1;
        if combatant.guard_recovery_remaining_ticks == 0 {
            combatant.guard_gauge = i32::try_from(phys.0.guard_break_threshold).unwrap_or(i32::MAX);
        }
    }
}

/// ADR-0028: `GuardBreak` は 1 frame の中継 state。`KinematicVel` は遷移時に
/// `guard_break_knockback` で既に充填済みなので、次フレームで `KnockbackUp` に書き換えて
/// ADR-0024 の吹っ飛びフローに合流させる。`Combatant.remaining_bounces` / `final_action` /
/// `hit_from_behind` は GuardBreak 遷移時に attack 解決側がリセット済み。
fn advance_guard_break(mut q: Query<&mut CharacterState, Without<HitStopState>>) {
    for mut state in &mut q {
        if matches!(*state, CharacterState::GuardBreak) {
            *state = CharacterState::KnockbackUp;
        }
    }
}

/// 吹っ飛びの空中ステージ (KnockbackUp/Down + BounceUp/Down) と Jump / JumpAttack
/// (ADR-0027) 中、Y 軸速度に重力を加算する。
/// `Physics.gravity` は「正値 = 落下方向」として書かれており、world Y は「上が +」なので
/// 加算ではなく **減算** で「下向きに加速」を表す。Slide / LieDown / Rise では gravity を
/// 適用しない (地面に乗っている)。dt は 60Hz 固定 (= `VSYNC_TICK_SECS`)。
fn apply_gravity(
    mut q: Query<(&CharacterState, &PhysicsParams, &mut KinematicVel), Without<HitStopState>>,
) {
    for (state, phys, mut vel) in &mut q {
        if matches!(
            *state,
            CharacterState::KnockbackUp
                | CharacterState::KnockbackDown
                | CharacterState::BounceUp
                | CharacterState::BounceDown
                | CharacterState::Jump
                | CharacterState::JumpAttack
        ) {
            // gravity は YAML で書ける f64 だが、実値は < ~1e4 (= 800 程度)。f32 mantissa
            // 23bit に十分収まるので truncation 警告は意図的に抑える。
            #[allow(clippy::cast_possible_truncation)]
            let g = phys.0.gravity as f32;
            vel.vel_y -= g * VSYNC_TICK_SECS;
        }
    }
}

/// `Slide` 中、XZ 速度を `ground_friction` で Coulomb 減速する。step を超える小さい速度は
/// 0 に clamp (符号反転で逆走しないように)。両軸 0 になった瞬間に `LieDown` に遷移して
/// `stage_timer_ticks` を充填する。`apply_velocity` の **前**に置いて semi-implicit な
/// 摩擦 → 積分にする (= friction を当 tick の vel に反映してから動かす)。
fn apply_slide_friction(
    mut q: Query<
        (
            &mut CharacterState,
            &mut KinematicVel,
            &PhysicsParams,
            &mut Combatant,
        ),
        Without<HitStopState>,
    >,
) {
    for (mut state, mut vel, phys, mut combatant) in &mut q {
        if !matches!(*state, CharacterState::Slide) {
            continue;
        }
        // friction は YAML で書ける f64 だが値域は重力と同様。f32 cast は意図的。
        #[allow(clippy::cast_possible_truncation)]
        let decel = phys.0.ground_friction as f32;
        vel.vel_x = apply_friction_step(vel.vel_x, decel, VSYNC_TICK_SECS);
        vel.vel_z = apply_friction_step(vel.vel_z, decel, VSYNC_TICK_SECS);
        // apply_friction_step が `<= step` で 0.0 に clamp するので、両方 0.0 で停止判定可。
        if vel.vel_x == 0.0 && vel.vel_z == 0.0 {
            combatant.stage_timer_ticks = ms_to_ticks(phys.0.lie_down_duration_ms);
            *state = CharacterState::LieDown;
        }
    }
}

/// `vel` を `decel * dt` で減速し、`|vel| <= step` なら 0 に clamp する Coulomb 摩擦 1 step。
/// 線形減衰ではなく Coulomb (絶対値で同じ量だけ削る) を使うのは、pixel art で「スッと
/// 止まる」感触を出すため。指数減衰だと停止に長くかかる。
#[must_use]
fn apply_friction_step(vel: f32, decel: f32, dt: f32) -> f32 {
    let step = decel * dt;
    if vel.abs() <= step {
        0.0
    } else {
        vel - step * vel.signum()
    }
}

/// 吹っ飛び中 (空中 + Slide) と Jump / JumpAttack は `KinematicVel` を `WorldPosition` に
/// 積分する。直前の `apply_gravity` / `apply_slide_friction` が当 frame の vel を更新して
/// いるので、semi-implicit Euler になる。Idle/Walk 等は通常 movement system が触る。
/// Jump / JumpAttack は X/Z 移動を handle_input 側が直接 `pos` に書く設計なので、ここでは
/// `vel.vel_y` だけが意味を持つ (= 重力での落下)。`vel.vel_x` / `vel.vel_z` も加算は走るが、
/// Jump 開始時に充填されていなければ 0 のまま。
fn apply_velocity(
    mut q: Query<(&CharacterState, &KinematicVel, &mut WorldPosition), Without<HitStopState>>,
) {
    for (state, vel, mut pos) in &mut q {
        if matches!(
            *state,
            CharacterState::KnockbackUp
                | CharacterState::KnockbackDown
                | CharacterState::BounceUp
                | CharacterState::BounceDown
                | CharacterState::Slide
                | CharacterState::Jump
                | CharacterState::JumpAttack
        ) {
            pos.x += vel.vel_x * VSYNC_TICK_SECS;
            pos.y += vel.vel_y * VSYNC_TICK_SECS;
            pos.z += vel.vel_z * VSYNC_TICK_SECS;
        }
    }
}

/// 頂点検知。上昇ステージ (`KnockbackUp` / `BounceUp`) 中で `vel_y <= 0` になったら
/// 対応する下降ステージ (`KnockbackDown` / `BounceDown`) に遷移する。
fn transition_at_apex(mut q: Query<(&mut CharacterState, &KinematicVel), Without<HitStopState>>) {
    for (mut state, vel) in &mut q {
        if vel.vel_y > 0.0 {
            continue;
        }
        let next = match *state {
            CharacterState::KnockbackUp => Some(CharacterState::KnockbackDown),
            CharacterState::BounceUp => Some(CharacterState::BounceDown),
            _ => None,
        };
        if let Some(s) = next {
            *state = s;
        }
    }
}

/// 着地検知。`KnockbackDown` / `BounceDown` 中で `y <= 0` になったら、
/// `remaining_bounces > 0` なら **Bounce** (vel_y 反転 + dampening、`BounceUp` へ、残数 -1)、
/// ゼロなら **Slide** (vel_y を 0 にして地面を滑る) に分岐する。pos.y は 0 に clamp。
///
/// Jump / JumpAttack 中で `y <= 0` (= 着地) のときは `Idle` に復帰し、`vel_y = 0` / `vel_x = 0`
/// / `vel_z = 0` にリセットする (ADR-0027)。空中での X/Z 移動は `handle_input` が直接 `pos` に
/// 書いているので vel は基本 0 のままだが、念のためここでクリアする。
fn detect_landing(
    mut q: Query<
        (
            &mut CharacterState,
            &mut WorldPosition,
            &mut KinematicVel,
            &PhysicsParams,
            &mut Combatant,
        ),
        Without<HitStopState>,
    >,
) {
    for (mut state, mut pos, mut vel, phys, mut combatant) in &mut q {
        let knockback_landing = matches!(
            *state,
            CharacterState::KnockbackDown | CharacterState::BounceDown
        ) && pos.y <= 0.0;
        let jump_landing = matches!(*state, CharacterState::Jump | CharacterState::JumpAttack)
            && pos.y <= 0.0
            // Jump 直後 (上昇中) の y <= 0 を誤検知しないよう、落下中 (vel_y <= 0) かつ
            // 出発点を一度離れた (= 1 tick 以上経過、上昇開始した) ことを vel_y <= 0 で代用。
            && vel.vel_y <= 0.0;
        if knockback_landing {
            pos.y = 0.0;
            if combatant.remaining_bounces > 0 {
                // Bounce: vel_y を反転して dampening、vel_x/vel_z も同 dampening。
                // bounce_dampening=0 は YAML 上「跳ねない」の表現としても使われるので、その場合は
                // 反転速度が 0 になり、次 tick で transition_at_apex が即 BounceDown に倒し、
                // 同 tick の detect_landing でまた処理される (= 残数を消費しながら最終的に Slide)。
                let damp = phys.0.bounce_dampening;
                vel.vel_x *= damp;
                vel.vel_z *= damp;
                vel.vel_y = -vel.vel_y * damp;
                combatant.remaining_bounces -= 1;
                *state = CharacterState::BounceUp;
            } else {
                // Slide: y 軸速度を 0、XZ は維持して Slide で摩擦減衰させる。
                vel.vel_y = 0.0;
                *state = CharacterState::Slide;
            }
        } else if jump_landing {
            pos.y = 0.0;
            vel.vel_x = 0.0;
            vel.vel_y = 0.0;
            vel.vel_z = 0.0;
            *state = CharacterState::Idle;
        }
    }
}

/// `LieDown` / `Rise` の固定 timer を 1 tick 進め、`stage_timer_ticks == 0` または
/// **anim 終端**で次段へ遷移する (ADR-0025 二重終了条件)。
/// - `LieDown → Rise`: stage_timer を `rise_duration_ms` 換算で充填、state を Rise に。
///   ただし `Combatant.final_action == Dead` なら遷移を抑止 (= LieDown 永続停止 / KO)。
/// - `Rise → Idle`: state を Idle に戻し、`gauge` / `remaining_bounces` / `final_action`
///   を初期値にリセット (= 起き上がった瞬間は次の被弾に向けて全部 fresh)。
fn advance_stage_timer(
    mut q: Query<
        (
            &mut CharacterState,
            &mut Combatant,
            &PhysicsParams,
            &AnimationFrames,
        ),
        Without<HitStopState>,
    >,
) {
    for (mut state, mut combatant, phys, anim) in &mut q {
        let next = match *state {
            CharacterState::LieDown => {
                if combatant.final_action == FinalAction::Dead {
                    // KO 演出: timer も anim 終端も無視して永続停止。
                    None
                } else {
                    let expired =
                        tick_down_or_anim_finished(&mut combatant.stage_timer_ticks, anim);
                    if expired {
                        combatant.stage_timer_ticks = ms_to_ticks(phys.0.rise_duration_ms);
                        Some(CharacterState::Rise)
                    } else {
                        None
                    }
                }
            }
            CharacterState::DownHit => {
                // 地上 hit が終わったら LieDown に戻し、stage_timer を fresh に
                // (= 倒れたまま、down 時間が延長される)。anim が is_loop=false なら finished
                // で進む。
                if anim.is_finished() {
                    combatant.stage_timer_ticks = ms_to_ticks(phys.0.lie_down_duration_ms);
                    Some(CharacterState::LieDown)
                } else {
                    None
                }
            }
            CharacterState::Rise => {
                let expired = tick_down_or_anim_finished(&mut combatant.stage_timer_ticks, anim);
                if expired {
                    combatant.gauge = i32::try_from(phys.0.knockback_threshold).unwrap_or(i32::MAX);
                    combatant.gauge_recovery_remaining_ticks = 0;
                    combatant.remaining_bounces = phys.0.bounce_count;
                    combatant.final_action = FinalAction::LieDown;
                    combatant.hit_from_behind = false;
                    // 1 コンボ終了 → ジャグル / DownHit counter を fresh に戻す。
                    combatant.juggle_count = 0;
                    combatant.down_hit_count = 0;
                    Some(CharacterState::Idle)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(s) = next {
            *state = s;
        }
    }
}

/// stage timer を 1 tick 進め、`0` に到達したか anim が末尾 frame を消化済みなら `true`。
/// 進める前に `0` なら anim 終端のみで終了判定する (= 固定 timer なし運用の fallback)。
fn tick_down_or_anim_finished(ticks: &mut u32, anim: &AnimationFrames) -> bool {
    if *ticks > 0 {
        *ticks -= 1;
    }
    *ticks == 0 || anim.is_finished()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_to_ticks_rounds_up_to_at_least_one() {
        assert_eq!(ms_to_ticks(0), 0);
        assert_eq!(ms_to_ticks(1), 1);
        assert_eq!(ms_to_ticks(16), 1);
        // 17ms 以上は 2 tick 以上 (16.67ms / tick)
        assert_eq!(ms_to_ticks(17), 2);
    }

    #[test]
    fn ms_to_ticks_matches_60hz_rate() {
        // 1 秒 = 60 tick
        assert_eq!(ms_to_ticks(1000), 60);
        // 800ms → ceil(48.0) = 48 tick
        assert_eq!(ms_to_ticks(800), 48);
        // 300ms → ceil(18.0) = 18 tick
        assert_eq!(ms_to_ticks(300), 18);
    }

    #[test]
    fn combatant_new_initializes_gauge_to_threshold() {
        let p = Physics {
            knockback_threshold: 120,
            bounce_count: 2,
            guard_break_threshold: 80,
            ..Physics::default()
        };
        let c = Combatant::new(&p);
        assert_eq!(c.gauge, 120);
        assert_eq!(c.gauge_recovery_remaining_ticks, 0);
        assert_eq!(c.stage_timer_ticks, 0);
        assert_eq!(c.remaining_bounces, 2);
        assert_eq!(c.final_action, FinalAction::default());
        assert!(!c.hit_from_behind);
        assert_eq!(c.guard_gauge, 80);
        assert_eq!(c.guard_recovery_remaining_ticks, 0);
    }

    #[test]
    fn final_action_default_is_lie_down() {
        // 致命傷でない通常の吹っ飛びは LieDown 終端 (Rise に進む)。Dead は attack 解決側が
        // 致命傷判定で明示的に立てる。
        assert_eq!(FinalAction::default(), FinalAction::LieDown);
    }

    #[test]
    fn apply_friction_step_clamps_to_zero_below_step() {
        // decel=100 px/s², dt=1/60s → step ≒ 1.67 px/s。それ以下は 0 に丸める。
        let after = apply_friction_step(1.0, 100.0, 1.0 / 60.0);
        assert!((after - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn apply_friction_step_subtracts_step_above_threshold() {
        // 10 px/s に対し step=1.67 → 10 - 1.67 ≒ 8.33。符号は維持。
        let after = apply_friction_step(10.0, 100.0, 1.0 / 60.0);
        assert!(after > 8.0 && after < 8.5);
        // 負方向も同様、絶対値が減る
        let after_neg = apply_friction_step(-10.0, 100.0, 1.0 / 60.0);
        assert!(after_neg > -8.5 && after_neg < -8.0);
    }

    #[test]
    fn apply_friction_step_preserves_zero_input() {
        // 元から 0 のものはそのまま 0。
        let after = apply_friction_step(0.0, 600.0, 1.0 / 60.0);
        assert!((after - 0.0).abs() < f32::EPSILON);
    }
}
