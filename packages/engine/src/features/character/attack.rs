//! Attack hit 判定 (FSD: feature slice)。
//!
//! ADR-0024 Phase A + B:
//! - [`AttackBox`] / [`BodyBox`] は world 座標 (画像ピクセル, ADR-0023) で表す軸並行ボックス。
//!   形状は YAML 駆動 (`Frame.attack_box_overrides`)。fallback ジオメトリは frame に geom が
//!   無いときの保険として残す。
//! - Player が [`CharacterState::Attack`] かつ frame に attack_meta があるとき
//!   (= `AnimationFrames::current_attack_meta` が `Some`) だけ player の前方に AttackBox を
//!   生やし、全 [`Enemy`] の BodyBox と AABB 判定する。
//! - ヒット時は `Combatant.gauge` を `meta.knockback_damage` で削り、`knockback_resistance`
//!   で減衰させる。次のいずれかで **吹っ飛び発動** (`CharacterState::KnockbackUp` 遷移):
//!     - gauge が枯れた (`<= 0`)
//!     - 空中被弾 (pos.y > 0)
//!     - 致命傷 (HP=0, Phase B)
//!   発動時は `KinematicVel` に Facing 反転 + attenuation 済みの knockback ベクトルを
//!   充填し、`remaining_bounces` / `gauge` を初期値に reset。致命傷なら `final_action=Dead`
//!   を立てて [`super::knockback::advance_stage_timer`] が LieDown で永続停止させる。
//! - 通常 Hit のときは `gauge_recovery_remaining_ticks` を立てる (間隔があけば自然回復)。
//! - 既に死亡 (HP=0) している enemy は AABB チェックを skip。
//! - 1 attack で同じ enemy を多重 hit しないよう、player に [`AttackHitConsumed`] フラグ。
use bevy::prelude::*;

use crate::entities::character::{AttackBoxMeta, KnockbackVec, Role};
use crate::shared::projection::{WorldBox, world_box_from_hitbox};

use super::animation::{AnimationFrames, AnimationSet};
use super::hit_stop::HitStopState;
use super::knockback::{Combatant, FinalAction, KinematicVel, PhysicsParams, ms_to_ticks};
use super::movement::{Enemy, Facing, Player, WorldPosition};
use super::state_machine::{CharacterState, EnemyAnimationSet};

/// player が今 attack の hit frame に居て、AttackBox が active な状態かを判定する。
/// 判定根拠は frame の `attack_box_overrides` の有無 (YAML 駆動)。`AnimationFrames` に
/// 焼き込まれた `current_attack_meta()` を見て、`Some` なら hit active と扱う。
/// `resolve_hits` (実際の当たり判定) と `hitbox_debug` (可視化) の両方から使う。
#[must_use]
pub fn is_attack_hit_active(state: CharacterState, anim: &AnimationFrames) -> bool {
    matches!(state, CharacterState::Attack) && anim.current_attack_meta().is_some()
}

/// player 中心から AttackBox 中心までの前方 X オフセット (画像ピクセル)。
const DEFAULT_ATTACK_OFFSET_X: f32 = 16.0;
/// Box の Y 中心を「足元 (= `pos.y`) からどれだけ持ち上げるか」。胴体中央あたり。
const DEFAULT_BOX_CENTER_Y: f32 = 30.0;

const DEFAULT_BODY_HALF_X: f32 = 12.0;
const DEFAULT_BODY_HALF_Y: f32 = 30.0;
const DEFAULT_BODY_HALF_Z: f32 = 8.0;
const DEFAULT_ATTACK_HALF_X: f32 = 12.0;
const DEFAULT_ATTACK_HALF_Y: f32 = 20.0;
const DEFAULT_ATTACK_HALF_Z: f32 = 8.0;

/// 被弾耐久。`current = 0` で `is_dead`、その瞬間に entity が despawn される (`resolve_hits` 側)。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitPoints {
    pub current: u32,
    pub max: u32,
}

impl HitPoints {
    #[must_use]
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    /// `amount` だけ減らし、underflow は 0 で打ち切る。
    pub fn damage(&mut self, amount: u32) {
        self.current = self.current.saturating_sub(amount);
    }

    #[must_use]
    pub fn is_dead(self) -> bool {
        self.current == 0
    }
}

/// player が「現在の attack window 中に既にヒットを 1 回消費したか」のフラグ。
/// Attack state に入った瞬間に false にリセットされ、ヒット解決で true に立つ。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AttackHitConsumed(pub bool);

/// キャラの world Z 全幅 (`Character.depth`)。HitBox.depth が `None` のときの
/// フォールバック値として `world_box_from_hitbox` に渡す。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterDepth(pub u32);

/// world 座標で表される軸並行ボックス (中心 + half-extent)。攻撃側。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackBox {
    pub center_x: f32,
    pub center_y: f32,
    pub center_z: f32,
    pub half_x: f32,
    pub half_y: f32,
    pub half_z: f32,
}

impl AttackBox {
    /// 攻撃側の現在位置と向きから AttackBox 中心を計算する (fallback)。
    /// `Facing::Right` で前方 = +X、`Facing::Left` で前方 = -X。
    /// 通常は YAML 駆動 (`from_world_box` 経由) が使われ、これは frame に
    /// `attack_box_overrides` が無い場合の保険として残してある。
    #[must_use]
    pub fn from_attacker(pos: WorldPosition, facing: Facing) -> Self {
        let dir = match facing {
            Facing::Right => 1.0,
            Facing::Left => -1.0,
        };
        Self {
            center_x: pos.x + dir * DEFAULT_ATTACK_OFFSET_X,
            center_y: pos.y + DEFAULT_BOX_CENTER_Y,
            center_z: pos.z,
            half_x: DEFAULT_ATTACK_HALF_X,
            half_y: DEFAULT_ATTACK_HALF_Y,
            half_z: DEFAULT_ATTACK_HALF_Z,
        }
    }

    /// `world_box_from_hitbox` で計算した [`WorldBox`] を AttackBox に詰め替える。
    /// AttackBox はマーカー的に独立した型として残しつつ、座標計算は projection 経由で共通化する。
    #[must_use]
    pub fn from_world_box(b: WorldBox) -> Self {
        Self {
            center_x: b.center_x,
            center_y: b.center_y,
            center_z: b.center_z,
            half_x: b.half_x,
            half_y: b.half_y,
            half_z: b.half_z,
        }
    }
}

/// world 座標で表される軸並行ボックス (中心 + half-extent)。被弾側。
/// Component として attach し、`sync_body_box` system が WorldPosition に追従させる。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct BodyBox {
    pub center_x: f32,
    pub center_y: f32,
    pub center_z: f32,
    pub half_x: f32,
    pub half_y: f32,
    pub half_z: f32,
}

impl BodyBox {
    /// キャラが地面 (`pos.y` = 0) に立っているときの default BodyBox を返す (fallback)。
    /// 通常は YAML 駆動 (`from_world_box`) が使われ、これは sprite に body_boxes が
    /// 1 つも無い場合の保険として残してある。
    #[must_use]
    pub fn default_for_world(pos: WorldPosition) -> Self {
        Self {
            center_x: pos.x,
            center_y: pos.y + DEFAULT_BOX_CENTER_Y,
            center_z: pos.z,
            half_x: DEFAULT_BODY_HALF_X,
            half_y: DEFAULT_BODY_HALF_Y,
            half_z: DEFAULT_BODY_HALF_Z,
        }
    }

    /// `world_box_from_hitbox` で計算した [`WorldBox`] を BodyBox に詰め替える。
    #[must_use]
    pub fn from_world_box(b: WorldBox) -> Self {
        Self {
            center_x: b.center_x,
            center_y: b.center_y,
            center_z: b.center_z,
            half_x: b.half_x,
            half_y: b.half_y,
            half_z: b.half_z,
        }
    }
}

/// XYZ 3 軸の AABB 重なり判定。境界 (面接触) は重なり扱い。
#[must_use]
pub fn aabb_intersects(a: &AttackBox, b: &BodyBox) -> bool {
    (a.center_x - b.center_x).abs() <= a.half_x + b.half_x
        && (a.center_y - b.center_y).abs() <= a.half_y + b.half_y
        && (a.center_z - b.center_z).abs() <= a.half_z + b.half_z
}

pub struct AttackPlugin;

impl Plugin for AttackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                reset_attack_hit_on_attack_start,
                sync_body_box,
                resolve_hits,
            )
                .chain()
                // sync_body_box は現 frame の geom を使うので、tick で frame が進んだ後に
                // 走らないと「旧 frame の body_box + 新 sprite」のミスマッチが起きる。
                .after(AnimationSet::Tick),
        );
    }
}

/// 毎 frame、`AnimationFrames` + `WorldPosition` + `Facing` + `CharacterDepth` から
/// 現在 frame の BodyBox 幾何を world XYZ に変換して `BodyBox` component に書き込む。
/// 先頭 body_box geom が無いときは `BodyBox::default_for_world` を fallback として使う。
fn sync_body_box(
    mut query: Query<(
        &WorldPosition,
        &Facing,
        &AnimationFrames,
        &CharacterDepth,
        &mut BodyBox,
    )>,
) {
    for (pos, facing, anim, depth, mut body) in &mut query {
        // movement::sync_transform と揃えて integer snap した世界座標で box を作る。
        // snap しないと sprite (snap 後 integer 位置) と body_box (sub-pixel) が
        // フレームごとに 1 px ずれるパターンが出るのと、collision も pixel grid 上で
        // 起きる方が pixel art の挙動として直感的。
        let (sx, sy, sz) = (pos.x.round(), pos.y.round(), pos.z.round());
        let new_box = match anim.current_body_boxes().first() {
            Some(geom) => BodyBox::from_world_box(world_box_from_hitbox(
                geom,
                anim.current_sprite_pivot(),
                sx,
                sy,
                sz,
                matches!(facing, Facing::Left),
                depth.0,
            )),
            None => BodyBox::default_for_world(WorldPosition::new(sx, sy, sz)),
        };
        // Bevy の `Changed<>` をぶらさないため等価チェックして必要なときだけ書く
        if *body != new_box {
            *body = new_box;
        }
    }
}

/// CharacterState が Attack に変わった瞬間 (Changed<CharacterState> で発火) に AttackHitConsumed を
/// false に戻す。これで「次の attack で再度 hit window を消費できる」状態になる。
fn reset_attack_hit_on_attack_start(
    mut query: Query<
        (&CharacterState, &mut AttackHitConsumed),
        (With<Player>, Changed<CharacterState>),
    >,
) {
    for (state, mut consumed) in &mut query {
        if matches!(state, CharacterState::Attack) {
            consumed.0 = false;
        }
    }
}

/// `KnockbackResistance` で attenuation = `(1 - clamp(resistance, 0, 1)).max(0)` を計算する。
/// damage / knockback_damage / 速度ベクトル全てに同じ係数を掛けて軽減する (ADR-0024)。
#[must_use]
fn attenuation(resistance: f32) -> f32 {
    (1.0 - resistance.clamp(0.0, 1.0)).max(0.0)
}

/// `meta.knockback` を attacker `Facing` で符号反転し、attenuation を掛けた効果速度を返す。
/// `Facing::Left` では vel_x の符号を反転する (`meta.vel_x` は「攻撃側前方 = +」基準)。
#[must_use]
fn effective_knockback(
    knockback: KnockbackVec,
    attacker_facing: Facing,
    attenuation: f32,
) -> KnockbackVec {
    let sign = match attacker_facing {
        Facing::Right => 1.0,
        Facing::Left => -1.0,
    };
    KnockbackVec {
        vel_x: knockback.vel_x * sign * attenuation,
        vel_y: knockback.vel_y * attenuation,
        vel_z: knockback.vel_z * attenuation,
    }
}

/// ADR-0025: 被弾者が自分の正面方向に飛ぶ = 背中を押された = 背後被弾。
/// `kb_vel_x` (= effective knockback の vel_x; 既に attacker Facing で符号反転済み) と
/// victim Facing の前方符号の積が正なら true。Vel_x = 0 (垂直のみの knockback) は false 扱い
/// (= 正面被弾と同等)。
#[must_use]
fn is_hit_from_behind(kb_vel_x: f32, victim_facing: Facing) -> bool {
    let defender_forward_sign = match victim_facing {
        Facing::Right => 1.0,
        Facing::Left => -1.0,
    };
    kb_vel_x * defender_forward_sign > 0.0
}

/// `meta` と attacker `Facing` / 被弾側 `Combatant` / `Physics` から、当 hit で発生する
/// 「damage / 吹っ飛び発動可否 / 効果速度」を決める。`Combatant.gauge` は副作用として
/// この関数内で削るが、knockback 発動時の **gauge リセットは呼び出し側**で行う。
struct HitOutcome {
    damage: u32,
    knockback: Option<KnockbackVec>,
}

fn decide_hit(
    meta: &AttackBoxMeta,
    attacker_facing: Facing,
    victim_pos_y: f32,
    combatant: &mut Combatant,
    phys: &PhysicsParams,
) -> HitOutcome {
    let att = attenuation(phys.0.knockback_resistance);
    // damage 値は u32 から f32 に乗せて係数を掛け、round で u32 に戻す。
    // YAML 由来値は < 1e6 で f32 mantissa に余裕、係数は [0, 1] なので overflow しない。
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let damage = (f32::from(u16::try_from(meta.damage).unwrap_or(u16::MAX)) * att).round() as u32;
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let kb_damage =
        (f32::from(u16::try_from(meta.knockback_damage).unwrap_or(u16::MAX)) * att).round() as i32;
    combatant.gauge = combatant.gauge.saturating_sub(kb_damage);
    let airborne = victim_pos_y > 0.0;
    let triggers_knockback = combatant.gauge <= 0 || airborne;
    let knockback =
        triggers_knockback.then(|| effective_knockback(meta.knockback, attacker_facing, att));
    HitOutcome { damage, knockback }
}

/// player が Attack の hit frame に居れば AttackBox を作り、全 Enemy の BodyBox と AABB 判定。
/// ヒットしたら `decide_hit` の結果で:
/// - 致命傷 (HP=0) → **knockback 強制発動 + `final_action = Dead`** (Phase B)。
///   `decide_hit` が knockback を返さなかった場合でも meta.knockback を attenuation して充填。
/// - knockback 発動 → `KinematicVel` 充填 + `CharacterState::KnockbackUp` + `Combatant.gauge`
///   を threshold に戻す + `remaining_bounces` を `max_bounce_count` に reset
/// - 通常 hit → `CharacterState::Hit` + `Combatant.gauge_recovery_remaining_ticks` を立てる
///
/// hit_stop は **通常 Hit 経路のみ** で適用する (knockback 発動時は hit_stop 無しで即遷移)。
/// 既に死亡 (HP=0) している enemy は AABB チェック前に skip し、二重 hit を防ぐ。
/// 1 attack で同 enemy を複数回叩かないよう、最初のヒットで [`AttackHitConsumed`] を立てる。
fn resolve_hits(
    mut commands: Commands,
    // Player / Enemy が同 entity に同時に attach されることは無い前提だが、Bevy の static
    // 解析は marker だけでは Query 間の disjoint を保証できないので、`Without` を明示する
    // (CharacterState を player は &, enemy は &mut で取るため B0001 を回避)。
    mut player_query: Query<
        (
            &WorldPosition,
            &Facing,
            &CharacterState,
            &AnimationFrames,
            &mut AttackHitConsumed,
            &CharacterDepth,
        ),
        (With<Player>, Without<Enemy>),
    >,
    mut enemy_query: Query<
        (
            Entity,
            &BodyBox,
            &WorldPosition,
            &Facing,
            &mut HitPoints,
            &mut CharacterState,
            &EnemyAnimationSet,
            &mut Combatant,
            &PhysicsParams,
            &mut KinematicVel,
        ),
        (With<Enemy>, Without<Player>),
    >,
    player_entity_query: Query<Entity, With<Player>>,
) {
    for (pos, facing, state, anim, mut consumed, depth) in &mut player_query {
        if !is_attack_hit_active(*state, anim) || consumed.0 {
            continue;
        }
        let Some(meta) = anim.current_attack_meta().copied() else {
            continue;
        };
        // sync_body_box / sync_transform と揃えて integer snap (pixel grid 上で当たり判定)。
        let (sx, sy, sz) = (pos.x.round(), pos.y.round(), pos.z.round());
        let attack_box = anim.current_attack_box_geom().map_or_else(
            || AttackBox::from_attacker(WorldPosition::new(sx, sy, sz), *facing),
            |geom| {
                AttackBox::from_world_box(world_box_from_hitbox(
                    geom,
                    anim.current_sprite_pivot(),
                    sx,
                    sy,
                    sz,
                    matches!(facing, Facing::Left),
                    depth.0,
                ))
            },
        );
        for (
            enemy_entity,
            body,
            enemy_pos,
            enemy_facing,
            mut hp,
            mut enemy_state,
            enemy_anims,
            mut combatant,
            phys,
            mut vel,
        ) in &mut enemy_query
        {
            // 既に死亡している enemy は AABB チェックすら不要。攻撃判定の二重発火と、
            // KO 演出 (LieDown 永続停止) 中の再被弾を両方避ける。
            if hp.is_dead() {
                continue;
            }
            if !aabb_intersects(&attack_box, body) {
                continue;
            }
            consumed.0 = true;
            let outcome = decide_hit(&meta, *facing, enemy_pos.y, &mut combatant, phys);
            hp.damage(outcome.damage);
            let lethal = hp.is_dead();
            tracing::info!(
                enemy = ?enemy_entity,
                damage = outcome.damage,
                remaining = hp.current,
                gauge = combatant.gauge,
                lethal,
                "attack: hit",
            );
            // 致命傷は decide_hit の判定 (gauge / 空中) を上書きして必ず knockback 発動。
            let knockback = outcome.knockback.or_else(|| {
                lethal.then(|| {
                    effective_knockback(
                        meta.knockback,
                        *facing,
                        attenuation(phys.0.knockback_resistance),
                    )
                })
            });
            if let Some(kb) = knockback {
                // 吹っ飛び発動: KinematicVel に充填、state を KnockbackUp に、gauge と
                // remaining_bounces を初期値に reset。lethal なら final_action=Dead を立て、
                // advance_stage_timer が LieDown 到達時に永続停止させる。
                // hit_from_behind は victim Facing と kb.vel_x の関係から判定 (ADR-0025):
                // 被弾者が自分の前方に飛ぶ = 背中側を押された。
                // knockback 経路では hit_stop を適用しない (Phase A の簡略化を維持)。
                vel.vel_x = kb.vel_x;
                vel.vel_y = kb.vel_y;
                vel.vel_z = kb.vel_z;
                combatant.gauge = i32::try_from(phys.0.knockback_threshold).unwrap_or(i32::MAX);
                combatant.gauge_recovery_remaining_ticks = 0;
                combatant.remaining_bounces = phys.0.max_bounce_count;
                combatant.final_action = if lethal {
                    FinalAction::Dead
                } else {
                    FinalAction::LieDown
                };
                combatant.hit_from_behind = is_hit_from_behind(kb.vel_x, *enemy_facing);
                *enemy_state = CharacterState::KnockbackUp;
                tracing::info!(
                    enemy = ?enemy_entity,
                    vel_x = kb.vel_x, vel_y = kb.vel_y, vel_z = kb.vel_z,
                    final_action = ?combatant.final_action,
                    hit_from_behind = combatant.hit_from_behind,
                    "attack: knockback triggered",
                );
            } else {
                // 通常 Hit (のけぞり): gauge_recovery_remaining_ticks を立てて、間隔を空ければ
                // 自然回復する。hit_stop は attack 側 meta.hit_stop の指定通りに発動。
                *enemy_state = CharacterState::Hit;
                combatant.gauge_recovery_remaining_ticks = ms_to_ticks(phys.0.hit_recovery_ms);
                if let Some(hs) = meta.hit_stop {
                    let fallback_ms = enemy_anims
                        .get(Role::Hit)
                        .and_then(|data| data.frames.first())
                        .map(|f| u32::try_from(f.duration.as_millis()).unwrap_or(u32::MAX));
                    if let Some(duration_ms) = hs.duration_ms.or(fallback_ms) {
                        commands.entity(enemy_entity).insert(HitStopState::victim(
                            duration_ms,
                            hs.shake_x,
                            hs.shake_y,
                            hs.count,
                            hs.decay,
                        ));
                        if let Ok(player_entity) = player_entity_query.single() {
                            commands
                                .entity(player_entity)
                                .insert(HitStopState::attacker(duration_ms));
                        }
                        tracing::info!(
                            duration_ms,
                            shake_x = hs.shake_x,
                            shake_y = hs.shake_y,
                            count = hs.count,
                            decay = hs.decay,
                            "attack: hit_stop applied",
                        );
                    }
                }
            }
            break; // 1 attack 1 hit: 同 frame で他の enemy には当てない
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn pos(x: f32, y: f32, z: f32) -> WorldPosition {
        WorldPosition::new(x, y, z)
    }

    #[test]
    fn attack_box_extends_forward_when_facing_right() {
        let p = pos(100.0, 0.0, 200.0);
        let b = AttackBox::from_attacker(p, Facing::Right);
        assert_eq!(b.center_x, 100.0 + DEFAULT_ATTACK_OFFSET_X);
        assert_eq!(b.center_y, DEFAULT_BOX_CENTER_Y);
        assert_eq!(b.center_z, 200.0);
    }

    #[test]
    fn attack_box_extends_backward_when_facing_left() {
        let p = pos(100.0, 0.0, 200.0);
        let b = AttackBox::from_attacker(p, Facing::Left);
        assert_eq!(b.center_x, 100.0 - DEFAULT_ATTACK_OFFSET_X);
    }

    #[test]
    fn body_box_centers_on_world_position_with_y_lift() {
        let b = BodyBox::default_for_world(pos(50.0, 0.0, 180.0));
        assert_eq!(b.center_x, 50.0);
        assert_eq!(b.center_y, DEFAULT_BOX_CENTER_Y);
        assert_eq!(b.center_z, 180.0);
    }

    #[test]
    fn aabb_intersects_when_player_attacks_adjacent_enemy() {
        // player (x=100, facing right) の AttackBox は 100 + 16 = 116 中心、half_x=12 → [104, 128]
        // enemy (x=130, body half_x=12) → [118, 142]。重なる範囲: [118, 128]。
        let attack = AttackBox::from_attacker(pos(100.0, 0.0, 200.0), Facing::Right);
        let body = BodyBox::default_for_world(pos(130.0, 0.0, 200.0));
        assert!(aabb_intersects(&attack, &body));
    }

    #[test]
    fn aabb_no_intersect_when_enemy_far_away_in_x() {
        let attack = AttackBox::from_attacker(pos(100.0, 0.0, 200.0), Facing::Right);
        let body = BodyBox::default_for_world(pos(200.0, 0.0, 200.0));
        assert!(!aabb_intersects(&attack, &body));
    }

    #[test]
    fn aabb_no_intersect_when_enemy_far_away_in_z() {
        // X は重なるが Z が離れていれば不命中 (奥/手前で外す)。
        let attack = AttackBox::from_attacker(pos(100.0, 0.0, 200.0), Facing::Right);
        let body = BodyBox::default_for_world(pos(120.0, 0.0, 150.0));
        assert!(!aabb_intersects(&attack, &body));
    }

    #[test]
    fn hit_points_new_initializes_full_health() {
        let hp = HitPoints::new(100);
        assert_eq!(hp.current, 100);
        assert_eq!(hp.max, 100);
        assert!(!hp.is_dead());
    }

    #[test]
    fn hit_points_damage_decrements_current() {
        let mut hp = HitPoints::new(100);
        hp.damage(30);
        assert_eq!(hp.current, 70);
    }

    #[test]
    fn hit_points_damage_saturates_at_zero() {
        let mut hp = HitPoints::new(20);
        hp.damage(50);
        assert_eq!(hp.current, 0);
        assert!(hp.is_dead());
    }

    use crate::entities::character::Physics;

    fn meta(damage: u32, knockback_damage: u32, vel_x: f32, vel_y: f32) -> AttackBoxMeta {
        AttackBoxMeta {
            damage,
            knockback_damage,
            knockback: KnockbackVec {
                vel_x,
                vel_y,
                vel_z: 0.0,
            },
            ..AttackBoxMeta::default()
        }
    }

    fn physics_with(resistance: f32, threshold: u32) -> PhysicsParams {
        PhysicsParams(Physics {
            knockback_resistance: resistance,
            knockback_threshold: threshold,
            ..Physics::default()
        })
    }

    fn combatant_with(gauge: i32) -> Combatant {
        Combatant {
            gauge,
            gauge_recovery_remaining_ticks: 0,
            stage_timer_ticks: 0,
            remaining_bounces: 0,
            final_action: FinalAction::default(),
            hit_from_behind: false,
        }
    }

    #[test]
    fn attenuation_full_at_zero_resistance() {
        assert!((attenuation(0.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn attenuation_zero_at_full_resistance() {
        assert!((attenuation(1.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn attenuation_clamps_below_zero() {
        // 不正な負 resistance も >1 と同じく clamp。
        assert!((attenuation(-0.5) - 1.0).abs() < f32::EPSILON);
        assert!((attenuation(2.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn effective_knockback_flips_x_when_attacker_faces_left() {
        let kb = KnockbackVec {
            vel_x: 120.0,
            vel_y: 80.0,
            vel_z: 0.0,
        };
        let right = effective_knockback(kb, Facing::Right, 1.0);
        assert!((right.vel_x - 120.0).abs() < f32::EPSILON);
        let left = effective_knockback(kb, Facing::Left, 1.0);
        assert!((left.vel_x + 120.0).abs() < f32::EPSILON);
        // vel_y は flip しない
        assert!((left.vel_y - 80.0).abs() < f32::EPSILON);
    }

    #[test]
    fn is_hit_from_behind_is_true_when_kb_pushes_in_facing_direction() {
        // Facing::Right の被弾者 (前方 = +X) が +X 方向に飛ぶ = 背中側を押された。
        assert!(is_hit_from_behind(120.0, Facing::Right));
        // Facing::Left の被弾者 (前方 = -X) が -X 方向に飛ぶ = 背中側を押された。
        assert!(is_hit_from_behind(-120.0, Facing::Left));
    }

    #[test]
    fn is_hit_from_behind_is_false_when_kb_pushes_against_facing() {
        // Facing::Right が -X 方向に飛ぶ = 正面から殴られて後ろに吹っ飛ばされた。
        assert!(!is_hit_from_behind(-120.0, Facing::Right));
        // Facing::Left が +X 方向に飛ぶ = 同じく正面被弾。
        assert!(!is_hit_from_behind(120.0, Facing::Left));
    }

    #[test]
    fn is_hit_from_behind_is_false_for_pure_vertical_knockback() {
        // 垂直のみ knockback (vel_x=0) は方向区別なし → 正面扱い。
        assert!(!is_hit_from_behind(0.0, Facing::Right));
        assert!(!is_hit_from_behind(0.0, Facing::Left));
    }

    #[test]
    fn effective_knockback_scales_by_attenuation() {
        let kb = KnockbackVec {
            vel_x: 100.0,
            vel_y: 50.0,
            vel_z: 20.0,
        };
        let half = effective_knockback(kb, Facing::Right, 0.5);
        assert!((half.vel_x - 50.0).abs() < f32::EPSILON);
        assert!((half.vel_y - 25.0).abs() < f32::EPSILON);
        assert!((half.vel_z - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn decide_hit_returns_no_knockback_when_gauge_remains() {
        let mut combatant = combatant_with(100);
        let phys = physics_with(0.0, 100);
        // 30 削っても gauge=70 残るので吹っ飛び発動しない (地上)
        let outcome = decide_hit(
            &meta(20, 30, 120.0, 80.0),
            Facing::Right,
            0.0,
            &mut combatant,
            &phys,
        );
        assert!(outcome.knockback.is_none());
        assert_eq!(outcome.damage, 20);
        assert_eq!(combatant.gauge, 70);
    }

    #[test]
    fn decide_hit_triggers_knockback_when_gauge_depleted() {
        let mut combatant = combatant_with(20);
        let phys = physics_with(0.0, 100);
        // 30 削ると gauge=-10 で発動
        let outcome = decide_hit(
            &meta(20, 30, 120.0, 80.0),
            Facing::Right,
            0.0,
            &mut combatant,
            &phys,
        );
        let kb = outcome.knockback.expect("knockback should fire");
        assert!((kb.vel_x - 120.0).abs() < f32::EPSILON);
        assert!(combatant.gauge <= 0);
    }

    #[test]
    fn decide_hit_triggers_knockback_when_airborne() {
        let mut combatant = combatant_with(100);
        let phys = physics_with(0.0, 100);
        // gauge は十分残っていても、空中被弾なら必ず発動
        let outcome = decide_hit(
            &meta(20, 1, 120.0, 80.0),
            Facing::Right,
            10.0,
            &mut combatant,
            &phys,
        );
        assert!(outcome.knockback.is_some());
    }

    #[test]
    fn decide_hit_applies_resistance_to_damage_and_knockback() {
        let mut combatant = combatant_with(100);
        // resistance=0.5 → attenuation=0.5
        let phys = physics_with(0.5, 100);
        let outcome = decide_hit(
            &meta(40, 60, 100.0, 80.0),
            Facing::Right,
            0.0,
            &mut combatant,
            &phys,
        );
        assert_eq!(outcome.damage, 20); // 40 * 0.5
        // gauge: 100 - (60 * 0.5) = 70
        assert_eq!(combatant.gauge, 70);
        // 70 > 0 で発動しない
        assert!(outcome.knockback.is_none());
    }

    #[test]
    fn aabb_intersect_boundary_is_inclusive() {
        // 面接触 (距離 = half_x + half_x) は重なり扱い。
        let attack = AttackBox {
            center_x: 0.0,
            center_y: DEFAULT_BOX_CENTER_Y,
            center_z: 0.0,
            half_x: 10.0,
            half_y: 10.0,
            half_z: 10.0,
        };
        let body = BodyBox {
            center_x: 20.0,
            center_y: DEFAULT_BOX_CENTER_Y,
            center_z: 0.0,
            half_x: 10.0,
            half_y: 10.0,
            half_z: 10.0,
        };
        assert!(aabb_intersects(&attack, &body));
    }
}
