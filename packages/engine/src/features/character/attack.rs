//! Attack hit 判定 (FSD: feature slice)。
//!
//! 最小実装方針:
//! - [`AttackBox`] / [`BodyBox`] は world 座標 (画像ピクセル, ADR-0023) で表す軸並行ボックス。
//!   Box の幾何 (中心 / half-extent) は今はまだ engine 内 default。YAML から画像 pixel →
//!   world XYZ への変換は次の踏み出しで導入する。
//! - Player が [`CharacterState::Attack`] かつ frame の `attack_box_overrides` が active な
//!   とき (= `AnimationFrames::current_attack_damage` が `Some`) だけ player の前方に
//!   AttackBox を生やし、全 [`Enemy`] の BodyBox と AABB 判定する。
//! - ヒットしたら `current_attack_damage()` のダメージ ([`DEFAULT_ATTACK_DAMAGE`] は YAML 未
//!   指定時の fallback) を [`HitPoints`] から減算し、HP > 0 なら `CharacterState::Hit` に
//!   遷移、HP 0 で entity を despawn。
//! - 1 attack で同じ enemy を多重 hit しないよう、player に [`AttackHitConsumed`] フラグを
//!   持たせ、Attack state に入った瞬間に reset する (= 1 attack あたり 1 hit window で
//!   1 ヒット限定)。吹っ飛び / Hit アニメは ADR-0024/0025 の本実装で別途。
use bevy::prelude::*;

use crate::entities::character::Role;
use crate::shared::projection::{WorldBox, world_box_from_hitbox};

use super::animation::{AnimationFrames, AnimationSet};
use super::hit_stop::HitStopState;
use super::movement::{Enemy, Facing, Player, WorldPosition};
use super::state_machine::{CharacterState, EnemyAnimationSet};

/// player が今 attack の hit frame に居て、AttackBox が active な状態かを判定する。
/// 判定根拠は frame の `attack_box_overrides` の有無 (YAML 駆動)。`AnimationFrames` に
/// 焼き込まれた `current_attack_damage()` を見て、`Some` なら hit active と扱う。
/// `resolve_hits` (実際の当たり判定) と `hitbox_debug` (可視化) の両方から使う。
#[must_use]
pub fn is_attack_hit_active(state: CharacterState, anim: &AnimationFrames) -> bool {
    matches!(state, CharacterState::Attack) && anim.current_attack_damage().is_some()
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

/// `AttackBoxMeta.damage` が YAML に書かれていない frame に当たった場合の fallback ダメージ。
/// 通常は YAML 側 (attack_box_overrides[0].meta.damage) が指定する。
pub const DEFAULT_ATTACK_DAMAGE: u32 = 50;

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

/// player が Attack の hit frame に居れば AttackBox を作り、全 Enemy の BodyBox と AABB 判定。
/// 当たった Enemy の HitPoints を [`DEFAULT_ATTACK_DAMAGE`] 減らし、HP 0 で despawn、
/// それ以外なら `CharacterState::Hit` に遷移させる (sync_enemy_animation が Hit アニメに差替え、
/// end_oneshot_actions が再生終了で Idle に戻す)。
/// 1 attack で同 enemy を複数回叩かないよう、最初のヒットで [`AttackHitConsumed`] を立てて
/// 同 attack window 中は判定を skip する (= 1 attack 1 hit)。
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
            &mut HitPoints,
            &mut CharacterState,
            &EnemyAnimationSet,
        ),
        (With<Enemy>, Without<Player>),
    >,
    player_entity_query: Query<Entity, With<Player>>,
) {
    for (pos, facing, state, anim, mut consumed, depth) in &mut player_query {
        if !is_attack_hit_active(*state, anim) || consumed.0 {
            continue;
        }
        let damage = anim
            .current_attack_damage()
            .unwrap_or(DEFAULT_ATTACK_DAMAGE);
        // YAML 駆動: attack_box_overrides[0].hitbox を world XYZ に変換して使う。
        // attack_box_geom が無い frame で hit window が立つことは想定外だが、
        // fallback として hardcoded geometry を維持する。
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
        let attack_hit_stop = anim.current_attack_hit_stop();
        for (enemy_entity, body, mut hp, mut enemy_state, enemy_anims) in &mut enemy_query {
            if !aabb_intersects(&attack_box, body) {
                continue;
            }
            hp.damage(damage);
            consumed.0 = true;
            tracing::info!(
                enemy = ?enemy_entity,
                damage,
                remaining = hp.current,
                "attack: hit",
            );
            if hp.is_dead() {
                tracing::info!(enemy = ?enemy_entity, "attack: enemy defeated");
                commands.entity(enemy_entity).despawn();
            } else {
                *enemy_state = CharacterState::Hit;
                // hit_stop decide: attack 側の hit_stop.duration_ms があればそれ、
                // 無ければ enemy の Hit アニメ frame 0 duration を fallback。両方無ければ
                // hit_stop なし (= 従来通り即 Hit アニメ再生)。
                if let Some(hs) = attack_hit_stop {
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
