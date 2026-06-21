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

use crate::entities::character::HitBox;
use crate::entities::project::Resolution;

/// world XYZ 軸並行ボックス。中心 + half-extent (画像 pixel = world 単位、ADR-0023)。
/// `attack` / `hitbox_debug` slice の AttackBox / BodyBox は同じ形を持つが、
/// 「マーカーとしての違い」を保つため別 type のままにしてある (CLAUDE.md: 先に共通化しない)。
/// 本構造は `world_box_from_hitbox` の戻り値 (中間表現) として使う。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldBox {
    pub center_x: f32,
    pub center_y: f32,
    pub center_z: f32,
    pub half_x: f32,
    pub half_y: f32,
    pub half_z: f32,
}

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
/// 画像 pixel で表された [`HitBox`] を、char の world 位置・最終 pivot・向き・depth から
/// world XYZ の [`WorldBox`] に変換する pure 関数 (ADR-0023)。
///
/// - 画像 X (左右) → world X (Facing::Left 時は pivot 中心で左右反転 = 画像 flip と整合)
/// - 画像 Y (下向き正) → world Y (上向き正) なので符号反転 (pivot 基準で上にあれば world Y+)
/// - depth は world Z の **全幅** で、center は `char_pos.z`、half は `depth / 2`
///   (Some なら hitbox.depth、None なら `char_depth` にフォールバック)
#[must_use]
// world_min/max_x_local と world_min/max_y_local は X/Y 軸ペアで対称命名が自然なので
// pedantic の similar_names は抑制する。
#[allow(clippy::similar_names)]
pub fn world_box_from_hitbox(
    hitbox: &HitBox,
    sprite_pivot: [i32; 2],
    char_pos_x: f32,
    char_pos_y: f32,
    char_pos_z: f32,
    facing_left: bool,
    char_depth: u32,
) -> WorldBox {
    let pivot_x = sprite_pivot[0] as f32;
    let pivot_y = sprite_pivot[1] as f32;
    let tl_x = hitbox.top_left[0] as f32 - pivot_x;
    let br_x = hitbox.bottom_right[0] as f32 - pivot_x;
    // 画像 X の min/max を Facing で反転。Facing::Left では右端と左端が世界では入れ替わる。
    let (world_min_x_local, world_max_x_local) = if facing_left {
        (-br_x, -tl_x)
    } else {
        (tl_x, br_x)
    };
    // 画像 Y は下向き正、world Y は上向き正なので pivot からの差を符号反転して取る。
    let world_min_y_local = pivot_y - hitbox.bottom_right[1] as f32;
    let world_max_y_local = pivot_y - hitbox.top_left[1] as f32;

    let world_min_x = char_pos_x + world_min_x_local;
    let world_max_x = char_pos_x + world_max_x_local;
    let world_min_y = char_pos_y + world_min_y_local;
    let world_max_y = char_pos_y + world_max_y_local;

    let depth = hitbox.depth.unwrap_or(char_depth) as f32;
    WorldBox {
        center_x: (world_min_x + world_max_x) * 0.5,
        center_y: (world_min_y + world_max_y) * 0.5,
        center_z: char_pos_z,
        half_x: (world_max_x - world_min_x) * 0.5,
        half_y: (world_max_y - world_min_y) * 0.5,
        half_z: depth * 0.5,
    }
}

#[must_use]
pub fn camera_translation(
    camera_start_x: i32,
    camera_start_y: i32,
    resolution: Resolution,
) -> Vec3 {
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

    fn hitbox(tl: [i32; 2], br: [i32; 2], depth: Option<u32>) -> HitBox {
        HitBox {
            top_left: tl,
            bottom_right: br,
            depth,
        }
    }

    #[test]
    fn world_box_from_hitbox_centers_on_char_with_pivot_offset() {
        // pivot=(20,90), hitbox=(top_left[28,30], bottom_right[52,50]) → 画像内で:
        //   X: pivot 右に +8..+32 → world X: +8..+32 (Facing Right)
        //   Y: pivot 上に 40..60 → world Y: +40..+60 (上向き正)
        // char_pos=(100, 0, 200), depth=Some(16) → half_z=8
        let hb = hitbox([28, 30], [52, 50], Some(16));
        let b = world_box_from_hitbox(&hb, [20, 90], 100.0, 0.0, 200.0, false, 16);
        assert!((b.center_x - 120.0).abs() < f32::EPSILON); // (108+132)/2
        assert!((b.half_x - 12.0).abs() < f32::EPSILON); // (132-108)/2
        assert!((b.center_y - 50.0).abs() < f32::EPSILON); // (40+60)/2
        assert!((b.half_y - 10.0).abs() < f32::EPSILON); // (60-40)/2
        assert!((b.center_z - 200.0).abs() < f32::EPSILON);
        assert!((b.half_z - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn world_box_from_hitbox_flips_x_when_facing_left() {
        // Facing::Left の場合、画像 X の +8..+32 が world では -32..-8 に反転する。
        let hb = hitbox([28, 30], [52, 50], Some(16));
        let b = world_box_from_hitbox(&hb, [20, 90], 100.0, 0.0, 200.0, true, 16);
        // world X 範囲: -32..-8 → center=-20 だが char_pos.x=100 を足して 80
        assert!((b.center_x - 80.0).abs() < f32::EPSILON);
        assert!((b.half_x - 12.0).abs() < f32::EPSILON);
        // Y は flip しないので Facing Right と同じ。
        assert!((b.center_y - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn world_box_from_hitbox_falls_back_to_char_depth_when_hitbox_depth_none() {
        let hb = hitbox([0, 0], [10, 10], None);
        let b = world_box_from_hitbox(&hb, [0, 0], 0.0, 0.0, 0.0, false, 24);
        assert!((b.half_z - 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn world_box_from_hitbox_uses_hitbox_depth_over_char_depth_when_present() {
        let hb = hitbox([0, 0], [10, 10], Some(4));
        let b = world_box_from_hitbox(&hb, [0, 0], 0.0, 0.0, 0.0, false, 24);
        assert!((b.half_z - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn camera_translation_centers_on_camera_start_plus_half_viewport() {
        // 0 + 384/2 = 192, -(8 + 216/2) = -116
        let res = Resolution {
            width: 384,
            height: 216,
        };
        let t = camera_translation(0, 8, res);
        assert_eq!(t, Vec3::new(192.0, -116.0, 0.0));
    }
}
