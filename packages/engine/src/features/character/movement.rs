//! Movement feature。矢印キー / WASD で `Player` の world 座標を更新し、
//! Camera を player に X 方向追従させ、`Facing` に応じて sprite を左右反転する。
//!
//! ADR-0023 の world 軸:
//! - 左右: `world_x` (+ = 右)
//! - 奥行: `world_z` (+ = 手前 = 画像下)
//! - 高さ: `world_y` (jump 用、本 system では触らない)
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::entities::level::Level;
use crate::entities::project::Project;
use crate::shared::projection;
use crate::shared::{Action, ActionMap};

use super::animation::{AnimationFrames, AnimationSet, VSYNC_TICK_SECS};
use super::debug_control::SimulationSet;
use super::knockback::{KinematicVel, PhysicsParams};
use super::state_machine::CharacterState;

/// Player を 1 体だけ識別する marker component。
#[derive(Component, Debug, Clone, Copy)]
pub struct Player;

/// Opponent (AI / 被弾対象) を識別する marker component。
/// `Player` と排他で、入力 system や camera follow からは除外される。
#[derive(Component, Debug, Clone, Copy)]
pub struct Enemy;

/// battle viewport を映している Camera2d を識別する marker component。
#[derive(Component, Debug, Clone, Copy)]
pub struct MainCamera;

/// ADR-0023 の world 座標を f32 で保持する component (連続移動用)。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct WorldPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl WorldPosition {
    #[must_use]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// player の向き。sprite の左右反転に焼き込む。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Facing {
    #[default]
    Right,
    Left,
}

/// 1 秒あたりの移動量 (画像ピクセル)。Beat 'em up のキャラ歩行はだいたい
/// 60-100 px/sec。後で Character.physics 由来にする想定で、現状は定数。
///
/// **60Hz pixel-perfect 補足**: 60 の整数倍 (60, 120, 180 ...) は毎 frame の
/// snap step が「常に同じ px 数」になって完全に滑らかに見える。
/// 非整数倍 (例: 80 = 1.333 px/frame) は snap pattern が `1, 2, 1, 1, 2 ...` の
/// 3-frame 周期になるが、AnimationSet::Tick 順序整理後はこの程度の周期パターンは
/// 体感的に許容できる。歩行速度の見た目を優先したい場合は 80 等の値も使ってよい。
const MOVE_SPEED_PX_PER_SEC: f32 = 80.0;

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        // 全部 Update に乗せて、handle_input 側で **dt を VSYNC_TICK_SECS 固定** にする。
        //
        // FixedUpdate は wall-clock accumulator なので、vsync の僅かなブレで
        // 1 render に 0 回 or 2 回走るドリフトが発生する (character の stall / jump として
        // 視認される)。Bevy の Update は vsync と 1:1 で走るので、Update 内で dt を
        // 固定値にするのが「frame = vsync = 1 step」を最も素直に実現できる。
        // 60Hz 想定 (animation の VSYNC_TICK と整合)。
        app.add_systems(
            Update,
            (
                handle_input,
                // sync_anchor / sync_flip は AnimationFrames の frame 切替後に走らないと
                // 「新 sprite.image + 旧 anchor/flip」の 1 frame ミスマッチが出る。
                // sync_transform / camera_follow は anim 非依存だが、tuple ごとまとめて
                // ordering を付けても害は無いのでそのまま after.
                (sync_transform, sync_flip, sync_anchor, camera_follow)
                    .after(handle_input)
                    .after(AnimationSet::Tick),
            )
                .in_set(SimulationSet::Active),
        );
    }
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    action_map: Res<ActionMap>,
    level: Option<Res<Level>>,
    mut query: Query<
        (
            &mut WorldPosition,
            &mut Facing,
            &mut CharacterState,
            &mut KinematicVel,
            &PhysicsParams,
        ),
        With<Player>,
    >,
) {
    // 60Hz 固定。`time.delta()` を読むと vsync ブレが乗って snap step pattern が
    // 不規則化するため、Update = vsync = 1 frame の前提で hardcode する。
    let dt = VSYNC_TICK_SECS;
    let step = MOVE_SPEED_PX_PER_SEC * dt;

    let mut dx = 0.0;
    let mut dz = 0.0;
    if action_map.pressed(&keys, Action::MoveRight) {
        dx += step;
    }
    if action_map.pressed(&keys, Action::MoveLeft) {
        dx -= step;
    }
    if action_map.pressed(&keys, Action::MoveDown) {
        dz += step;
    }
    if action_map.pressed(&keys, Action::MoveUp) {
        dz -= step;
    }
    let attack_pressed = action_map.just_pressed(&keys, Action::Attack);
    // 下段攻撃 (倒れた敵向けの低位置 AttackBox)。
    let down_attack_pressed = action_map.just_pressed(&keys, Action::DownAttack);
    // ジャンプ (ADR-0027)、地上 (pos.y == 0) でのみ受付。
    let jump_pressed = action_map.just_pressed(&keys, Action::Jump);
    // ガード (ADR-0028): 押下中だけ維持。離すと Idle に戻る (Guard 経路で処理)。
    let guard_pressed = action_map.pressed(&keys, Action::Guard);
    let move_target_state = if dx == 0.0 && dz == 0.0 {
        CharacterState::Idle
    } else {
        CharacterState::Walk
    };
    // Level 未設定なら制限なし扱い (ADR-0022 の fail-soft)
    let contains = |x: f32, z: f32| level.as_deref().is_none_or(|l| l.contains_xz(x, z));
    for (mut pos, mut facing, mut state, mut vel, phys) in &mut query {
        // Jump 中 (非 locked): 空中移動と向き更新を許し、attack キーで JumpAttack へ。
        // Y 軸物理は knockback::apply_gravity / apply_velocity / detect_landing が回す。
        if matches!(*state, CharacterState::Jump) {
            if dx != 0.0 || dz != 0.0 {
                let next = step_axis_aware(*pos, dx, dz, contains);
                pos.x = next.x;
                pos.z = next.z;
                if dx > 0.0 {
                    *facing = Facing::Right;
                } else if dx < 0.0 {
                    *facing = Facing::Left;
                }
            }
            if attack_pressed {
                *state = CharacterState::JumpAttack;
            }
            continue;
        }
        if state.is_locked() {
            // Attack / JumpAttack / Hit / 吹っ飛びフロー / GuardBreak は入力で上書きしない。
            continue;
        }
        // Guard 中 (非 locked): L 離すと Idle、押下続いていれば Guard 維持。
        // ガード中は移動 / 攻撃 / ジャンプ入力を全て無視 (= 完全停止)。
        if matches!(*state, CharacterState::Guard) {
            if !guard_pressed {
                *state = CharacterState::Idle;
            }
            continue;
        }
        // 地上の通常入力。優先度: Guard > Jump > Attack > DownAttack > 移動。
        // Guard と Jump は地上 (pos.y == 0) のみ受付 (空中ガード / 二段ジャンプ無し、ADR-0027/0028)。
        if guard_pressed && pos.y == 0.0 {
            *state = CharacterState::Guard;
            continue;
        }
        if jump_pressed && pos.y == 0.0 {
            // Y 速度に jump_velocity_y を充填して上昇開始。X/Z は当 frame の入力で別途進む。
            #[allow(clippy::cast_possible_truncation)]
            let jv = phys.0.jump_velocity_y as f32;
            vel.vel_y = jv;
            *state = CharacterState::Jump;
            continue;
        }
        if attack_pressed {
            *state = CharacterState::Attack;
            continue;
        }
        if down_attack_pressed {
            *state = CharacterState::DownAttack;
            continue;
        }
        if dx != 0.0 || dz != 0.0 {
            let next = step_axis_aware(*pos, dx, dz, contains);
            pos.x = next.x;
            pos.z = next.z;
            // 左右入力があるときだけ向きを更新 (上下のみだと向きを維持)
            if dx > 0.0 {
                *facing = Facing::Right;
            } else if dx < 0.0 {
                *facing = Facing::Left;
            }
        }
        // Bevy の Changed<> をぶれずに発火させるため等価チェックして必要なときだけ書く
        if *state != move_target_state {
            *state = move_target_state;
        }
    }
}

/// X 軸 → Z 軸の順に試して、壁にぶつかった軸だけ移動量を捨てる。
/// 対角入力で台形 area の斜辺に沿って滑る挙動を出す。
#[must_use]
pub fn step_axis_aware(
    pos: WorldPosition,
    dx: f32,
    dz: f32,
    contains: impl Fn(f32, f32) -> bool,
) -> WorldPosition {
    let mut next = pos;
    let candidate_x = next.x + dx;
    if contains(candidate_x, next.z) {
        next.x = candidate_x;
    }
    let candidate_z = next.z + dz;
    if contains(next.x, candidate_z) {
        next.z = candidate_z;
    }
    next
}

/// WorldPosition は f32 連続値だが、画像描画は Nearest filter (Bevy default) なので
/// transform を整数 pixel に snap してから projection に渡す。
/// snap 無しだと world x の sub-pixel 揺らぎが nearest snap の境界 (.5) をまたぐ
/// 瞬間にだけ sprite が 1 px 動いて見え、vsync の delta jitter と相互作用して
/// 「frame ごとに 1px or 2px の step が不均一」に見える (= 横揺れ感)。
/// 整数 snap すれば step は `1, 1, 2, 1, 1, 2 ...` 等の規則的パターンになり安定する。
fn sync_transform(mut query: Query<(&WorldPosition, &mut Transform), Changed<WorldPosition>>) {
    for (pos, mut transform) in &mut query {
        transform.translation =
            projection::world_to_bevy_f32(pos.x.round(), pos.y.round(), pos.z.round());
    }
}

/// 最終 flip_x = Facing flip XOR Animation 側 flip (frame.flip XOR layer.flip 済み)。
fn sync_flip(
    mut query: Query<
        (&Facing, &AnimationFrames, &mut Sprite),
        Or<(Changed<Facing>, Changed<AnimationFrames>)>,
    >,
) {
    for (facing, anim, mut sprite) in &mut query {
        sprite.flip_x = total_flip_x(*facing, anim.current_flip_x());
    }
}

/// `Facing` または `AnimationFrames`（frame 切替）が変わったら、現フレームの
/// base anchor を「最終 flip_x」に応じて x 反転して `Anchor` に書く。
fn sync_anchor(
    mut query: Query<
        (&AnimationFrames, &Facing, &mut Anchor),
        Or<(Changed<AnimationFrames>, Changed<Facing>)>,
    >,
) {
    for (anim, facing, mut anchor) in &mut query {
        let flip_x = total_flip_x(*facing, anim.current_flip_x());
        *anchor = flip_anchor(anim.current_anchor(), flip_x);
    }
}

/// Facing flip と Animation 側 flip を XOR する pure 関数。
#[must_use]
pub fn total_flip_x(facing: Facing, anim_flip_x: bool) -> bool {
    let facing_flip = matches!(facing, Facing::Left);
    facing_flip ^ anim_flip_x
}

/// `flip_x = true` のとき anchor.x の符号を反転する pure 関数。
#[must_use]
pub fn flip_anchor(base: Anchor, flip_x: bool) -> Anchor {
    if flip_x {
        Anchor(Vec2::new(-base.0.x, base.0.y))
    } else {
        base
    }
}

fn camera_follow(
    project: Option<Res<Project>>,
    player: Query<&WorldPosition, (With<Player>, Without<MainCamera>)>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(project) = project else {
        return;
    };
    let Ok(player_pos) = player.single() else {
        return;
    };
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let half_view = project.resolution.width as f32 / 2.0;
    // sync_transform と同じ理由で整数 snap。player.x は f32 のまま保持し、camera と
    // player 両方を同じ integer pixel に揃えることで両者の相対位置を sub-pixel ズレ
    // させない (沿わせないと sprite の nearest snap で 1px ぶれる)。
    transform.translation.x = clamp_camera_x(player_pos.x, half_view).round();
}

/// camera が world 左端を越えて左に行かないよう player の X を clamp する。
/// (右端 clamp は base 画像の幅を知る必要があるので別段階。)
#[must_use]
pub fn clamp_camera_x(player_x: f32, half_view_width: f32) -> f32 {
    player_x.max(half_view_width)
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // bit-exact 比較 (Default / new() / clamp の直接代入)
mod tests {
    use super::*;

    #[test]
    fn world_position_default_is_origin() {
        let p = WorldPosition::default();
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 0.0);
        assert_eq!(p.z, 0.0);
    }

    #[test]
    fn world_position_new_stores_components() {
        let p = WorldPosition::new(28.0, 0.0, 180.0);
        assert_eq!(p.x, 28.0);
        assert_eq!(p.y, 0.0);
        assert_eq!(p.z, 180.0);
    }

    #[test]
    fn facing_default_is_right() {
        assert_eq!(Facing::default(), Facing::Right);
    }

    #[test]
    fn clamp_camera_x_lets_camera_track_player_inside_view() {
        // player が view 半幅より右にいるときはそのまま追従
        assert_eq!(clamp_camera_x(500.0, 192.0), 500.0);
    }

    #[test]
    fn clamp_camera_x_pins_camera_at_left_edge() {
        // player が view 半幅より左 (world 左端付近) では camera を pin
        assert_eq!(clamp_camera_x(50.0, 192.0), 192.0);
        assert_eq!(clamp_camera_x(0.0, 192.0), 192.0);
    }

    #[test]
    fn flip_anchor_identity_when_not_flipped() {
        let base = Anchor(Vec2::new(0.12, -0.48));
        assert_eq!(flip_anchor(base, false), base);
    }

    #[test]
    fn flip_anchor_inverts_x_only_when_flipped() {
        let base = Anchor(Vec2::new(0.12, -0.48));
        let flipped = flip_anchor(base, true);
        assert_eq!(flipped.0.x, -0.12);
        assert_eq!(flipped.0.y, -0.48);
    }

    #[test]
    fn total_flip_x_xors_facing_and_animation() {
        assert!(!total_flip_x(Facing::Right, false));
        assert!(total_flip_x(Facing::Left, false));
        assert!(total_flip_x(Facing::Right, true));
        // 両方 flip だと打ち消し合って元に戻る
        assert!(!total_flip_x(Facing::Left, true));
    }

    #[test]
    fn step_axis_aware_moves_both_axes_when_unrestricted() {
        let pos = WorldPosition::new(10.0, 0.0, 20.0);
        let next = step_axis_aware(pos, 5.0, -3.0, |_, _| true);
        assert_eq!(next.x, 15.0);
        assert_eq!(next.z, 17.0);
    }

    #[test]
    fn step_axis_aware_drops_only_blocked_axis() {
        // x > 12 を outside、それ以外 inside。x 方向だけ拒否される。
        let pos = WorldPosition::new(10.0, 0.0, 20.0);
        let next = step_axis_aware(pos, 5.0, -3.0, |x, _| x <= 12.0);
        assert_eq!(next.x, 10.0);
        assert_eq!(next.z, 17.0);
    }

    #[test]
    fn step_axis_aware_holds_position_in_corner() {
        let pos = WorldPosition::new(10.0, 0.0, 20.0);
        let next = step_axis_aware(pos, 5.0, 5.0, |_, _| false);
        assert_eq!(next.x, 10.0);
        assert_eq!(next.z, 20.0);
    }
}
