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

use super::animation::AnimationFrames;
use super::state_machine::PlayerState;

/// Player を 1 体だけ識別する marker component。
#[derive(Component, Debug, Clone, Copy)]
pub struct Player;

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
const MOVE_SPEED_PX_PER_SEC: f32 = 80.0;

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_input,
                (sync_transform, sync_flip, sync_anchor, camera_follow).after(handle_input),
            ),
        );
    }
}

fn handle_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    level: Option<Res<Level>>,
    mut query: Query<(&mut WorldPosition, &mut Facing, &mut PlayerState), With<Player>>,
) {
    let dt = time.delta_secs();
    let step = MOVE_SPEED_PX_PER_SEC * dt;

    let mut dx = 0.0;
    let mut dz = 0.0;
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        dx += step;
    }
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        dx -= step;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        dz += step;
    }
    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        dz -= step;
    }
    let target_state = if dx == 0.0 && dz == 0.0 {
        PlayerState::Idle
    } else {
        PlayerState::Walk
    };
    // Level 未設定なら制限なし扱い (ADR-0022 の fail-soft)
    let contains = |x: f32, z: f32| level.as_deref().is_none_or(|l| l.contains_xz(x, z));
    for (mut pos, mut facing, mut state) in &mut query {
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
        if *state != target_state {
            *state = target_state;
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

fn sync_transform(mut query: Query<(&WorldPosition, &mut Transform), Changed<WorldPosition>>) {
    for (pos, mut transform) in &mut query {
        transform.translation = projection::world_to_bevy_f32(pos.x, pos.y, pos.z);
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
    transform.translation.x = clamp_camera_x(player_pos.x, half_view);
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
