//! Hitbox debug overlay (FSD: feature slice)。
//!
//! `F1` で [`HitboxDebugEnabled`] を toggle し、有効時に Bevy `Gizmos` で
//! BodyBox / Active な AttackBox を world XY 平面の枠線として描く (ADR-0023 投影)。
//! 矩形は box の (center_x ± half_x, center_y ± half_y) の 4 角を取り、Z の影響は
//! `projection::world_to_bevy_f32` に任せて Bevy Y の縦シフトに変換する。
//!
//! AttackBox は [`is_attack_hit_active`] が true な player frame のときだけ赤で描く。
//! BodyBox は常時緑で描く。色はとりあえず固定 (デバッグ用、将来 invincibility 表現を
//! 入れるなら state 別に塗り分ける余地あり)。
use bevy::prelude::*;

use crate::shared::projection::{self, world_box_from_hitbox};

use super::animation::{AnimationFrames, AnimationSet};
use super::attack::{AttackBox, BodyBox, CharacterDepth, is_attack_hit_active};
use super::movement::{Enemy, Facing, Player, WorldPosition};
use super::state_machine::CharacterState;

const BODY_COLOR: Color = Color::srgb(0.3, 1.0, 0.4);
/// `BodyBox.disabled=true` (ADR-0024 BodyBox-driven 無敵 frame) を区別するため、
/// 灰色っぽい色で出す。アニメ author が「無敵が立ったタイミング」を視認できる。
const BODY_DISABLED_COLOR: Color = Color::srgb(0.5, 0.5, 0.55);
const ATTACK_COLOR: Color = Color::srgb(1.0, 0.3, 0.3);
const PIVOT_COLOR: Color = Color::srgb(1.0, 1.0, 0.2);
/// pivot marker のクロス半径 (world px = 画像 px)。低解像度 (384×216) で潰れない最小値。
const PIVOT_HALF_PX: f32 = 3.0;

/// Hitbox overlay の on/off。F1 で toggle。default = off。
#[derive(Resource, Debug, Default)]
pub struct HitboxDebugEnabled(pub bool);

pub struct HitboxDebugPlugin;

impl Plugin for HitboxDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HitboxDebugEnabled>().add_systems(
            Update,
            (toggle_debug, draw_hitboxes, draw_pivots)
                .chain()
                // pivot marker / hitbox 描画は anim 現 frame の値を使うので、
                // sync_body_box と同じく tick の後に走らせる必要がある。
                .after(AnimationSet::Tick),
        );
    }
}

fn toggle_debug(keys: Res<ButtonInput<KeyCode>>, mut enabled: ResMut<HitboxDebugEnabled>) {
    if keys.just_pressed(KeyCode::F1) {
        enabled.0 = !enabled.0;
        tracing::info!(enabled = enabled.0, "hitbox debug: toggled");
    }
}

fn draw_hitboxes(
    enabled: Res<HitboxDebugEnabled>,
    mut gizmos: Gizmos,
    body_query: Query<&BodyBox>,
    player_query: Query<
        (
            &WorldPosition,
            &Facing,
            &CharacterState,
            &AnimationFrames,
            &CharacterDepth,
        ),
        (With<Player>, Without<Enemy>),
    >,
) {
    if !enabled.0 {
        return;
    }
    for body in &body_query {
        let color = if body.disabled {
            BODY_DISABLED_COLOR
        } else {
            BODY_COLOR
        };
        draw_box_xy(
            &mut gizmos,
            body.center_x,
            body.center_y,
            body.center_z,
            body.half_x,
            body.half_y,
            color,
        );
    }
    for (pos, facing, state, anim, depth) in &player_query {
        if !is_attack_hit_active(*state, anim) {
            continue;
        }
        // resolve_hits と同じ AttackBox 計算 (YAML 駆動)。geom が無いときは fallback。
        let ab = anim.current_attack_box_geom().map_or_else(
            || AttackBox::from_attacker(*pos, *facing),
            |geom| {
                AttackBox::from_world_box(world_box_from_hitbox(
                    geom,
                    anim.current_sprite_pivot(),
                    pos.x,
                    pos.y,
                    pos.z,
                    matches!(facing, Facing::Left),
                    depth.0,
                ))
            },
        );
        draw_box_xy(
            &mut gizmos,
            ab.center_x,
            ab.center_y,
            ab.center_z,
            ab.half_x,
            ab.half_y,
            ATTACK_COLOR,
        );
    }
}

/// 各キャラの pivot point (= world position) を黄色いクロスで描く。
/// pivot は image の `pivot_point` 画素が world position に anchor された点なので、
/// 単純に `world_to_bevy_f32(pos)` の位置にクロスを置けばよい。
/// pivot 揺れによる lurch を視覚的に追うためのデバッグ用。
fn draw_pivots(
    enabled: Res<HitboxDebugEnabled>,
    mut gizmos: Gizmos,
    query: Query<&WorldPosition, With<AnimationFrames>>,
) {
    if !enabled.0 {
        return;
    }
    for pos in &query {
        // sync_transform と同じ snap を入れる (= sprite が実際に描画される位置に marker を合わせる)。
        let c = projection::world_to_bevy_f32(pos.x.round(), pos.y.round(), pos.z.round());
        gizmos.line(
            c + Vec3::new(-PIVOT_HALF_PX, 0.0, 0.0),
            c + Vec3::new(PIVOT_HALF_PX, 0.0, 0.0),
            PIVOT_COLOR,
        );
        gizmos.line(
            c + Vec3::new(0.0, -PIVOT_HALF_PX, 0.0),
            c + Vec3::new(0.0, PIVOT_HALF_PX, 0.0),
            PIVOT_COLOR,
        );
    }
}

fn draw_box_xy(
    gizmos: &mut Gizmos,
    center_x: f32,
    center_y: f32,
    center_z: f32,
    half_x: f32,
    half_y: f32,
    color: Color,
) {
    let corners = box_corners_xy(center_x, center_y, half_x, half_y);
    let screen: [Vec3; 4] = corners.map(|(wx, wy)| projection::world_to_bevy_f32(wx, wy, center_z));
    for i in 0..4 {
        gizmos.line(screen[i], screen[(i + 1) % 4], color);
    }
}

/// 矩形 (XY 平面、Z は無視) の 4 頂点を CCW 順で返す。
/// 返り値は `(bl, br, tr, tl)` で、line を `[i] -> [(i+1)%4]` で繋ぐと枠線になる。
#[must_use]
pub fn box_corners_xy(center_x: f32, center_y: f32, half_x: f32, half_y: f32) -> [(f32, f32); 4] {
    [
        (center_x - half_x, center_y - half_y),
        (center_x + half_x, center_y - half_y),
        (center_x + half_x, center_y + half_y),
        (center_x - half_x, center_y + half_y),
    ]
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn box_corners_xy_returns_four_corners_in_ccw() {
        let c = box_corners_xy(100.0, 50.0, 10.0, 20.0);
        assert_eq!(c[0], (90.0, 30.0)); // bottom-left
        assert_eq!(c[1], (110.0, 30.0)); // bottom-right
        assert_eq!(c[2], (110.0, 70.0)); // top-right
        assert_eq!(c[3], (90.0, 70.0)); // top-left
    }

    #[test]
    fn box_corners_xy_zero_half_is_a_single_point() {
        let c = box_corners_xy(7.0, 13.0, 0.0, 0.0);
        for corner in c {
            assert_eq!(corner, (7.0, 13.0));
        }
    }
}
