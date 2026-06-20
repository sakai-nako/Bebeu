//! ADR-0023 に従う world ↔ Bevy world 座標変換。
//!
//! world は「base 画像ピクセル座標」を直接使う:
//!
//! - `world_x` = base 画像ピクセル X (X+ = 右)
//! - `world_z` = base 画像ピクセル Y (Z+ = 画像下 = 手前)
//! - `world_y` = 高さ (Y+ = 上、0 = 地面、jump 用に予約)
//!
//! Bevy 2D world は (X+ = 右, Y+ = 上, Z = 描画順) なので、Y/Z 軸の符号を反転して
//! Bevy world に乗せる。Camera は viewport の左上を world の `camera_start_*` に
//! 合わせる位置に置く。
use bevy::math::Vec3;

use crate::entities::project::Resolution;

/// world 座標 (画像ピクセル系) を Bevy world Transform 用 Vec3 に変換する。
///
/// - bevy_x = world_x
/// - bevy_y = world_y - world_z (高さで上、奥行きで下)
/// - bevy_z = world_z (z-order: world_z 大 = 手前 → bevy z 大)
#[must_use]
pub fn world_to_bevy(world_x: i32, world_y: i32, world_z: i32) -> Vec3 {
    world_to_bevy_f32(world_x as f32, world_y as f32, world_z as f32)
}

/// `world_to_bevy` の f32 版。連続移動 (毎フレーム delta 加算) で使う。
#[must_use]
pub fn world_to_bevy_f32(world_x: f32, world_y: f32, world_z: f32) -> Vec3 {
    Vec3::new(world_x, world_y - world_z, world_z)
}

/// `camera_start` (視界左上隅の画像ピクセル座標) と viewport から、
/// Bevy Camera2d の `Transform.translation` を返す。
///
/// Bevy の Camera2d は中央を見るため、視界中央 = (camera_start + viewport/2) になるよう
/// 平行移動する。Y は world (画像ピクセル Y+ 下) と Bevy (Y+ 上) で符号反転。
#[must_use]
pub fn camera_translation(camera_start_x: i32, camera_start_y: i32, resolution: Resolution) -> Vec3 {
    Vec3::new(
        camera_start_x as f32 + resolution.width as f32 / 2.0,
        -(camera_start_y as f32 + resolution.height as f32 / 2.0),
        0.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_to_bevy_zero_is_origin() {
        let v = world_to_bevy(0, 0, 0);
        assert_eq!(v, Vec3::ZERO);
    }

    #[test]
    fn world_to_bevy_z_pulls_down_in_bevy_y() {
        // world_z (画像下 = 手前) は bevy world で Y- に来る (画面下方向)
        let v = world_to_bevy(10, 0, 50);
        assert_eq!(v, Vec3::new(10.0, -50.0, 50.0));
    }

    #[test]
    fn world_to_bevy_y_pushes_up_in_bevy_y() {
        // world_y (高さ) は bevy world Y+ にそのまま (ジャンプは上昇)
        let v = world_to_bevy(0, 30, 0);
        assert_eq!(v, Vec3::new(0.0, 30.0, 0.0));
    }

    #[test]
    fn world_to_bevy_y_and_z_compose_independently() {
        // jump (Y+30) しながら手前 (Z+50) → bevy y = 30 - 50 = -20
        let v = world_to_bevy(7, 30, 50);
        assert_eq!(v, Vec3::new(7.0, -20.0, 50.0));
    }

    #[test]
    fn world_to_bevy_f32_is_consistent_with_i32_variant() {
        // 整数値で評価すれば f32 / i32 版が一致する。
        let a = world_to_bevy(10, 20, 30);
        let b = world_to_bevy_f32(10.0, 20.0, 30.0);
        assert_eq!(a, b);
    }

    #[test]
    fn world_to_bevy_f32_preserves_fractional_position() {
        // 連続移動 (subpixel) でも壊れない: y = 12.5 - 7.25 = 5.25
        let v = world_to_bevy_f32(3.5, 12.5, 7.25);
        assert_eq!(v, Vec3::new(3.5, 5.25, 7.25));
    }

    #[test]
    fn camera_translation_centers_on_camera_start_plus_half_viewport() {
        // 0 + 384/2 = 192, -(8 + 216/2) = -116
        let res = Resolution { width: 384, height: 216 };
        let t = camera_translation(0, 8, res);
        assert_eq!(t, Vec3::new(192.0, -116.0, 0.0));
    }
}
