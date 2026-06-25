//! RGB <-> HSV 変換 (color picker UI 用)。
//!
//! HexColor は domain 型なので HSV 変換はここに分離する。

/// HSV 色空間の値。`h` は 0..=360 (deg)、`s` / `v` は 0.0..=1.0。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

/// 0..=255 の RGB から HSV へ。Saturation/Value はゼロ近傍で hue が不定になるが、
/// その場合は h=0 を返す (UI 上の slider 位置を安定させる)。
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> Hsv {
    let r = f32::from(r) / 255.0;
    let g = f32::from(g) / 255.0;
    let b = f32::from(b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta == 0.0 {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { delta / max };
    Hsv { h, s, v: max }
}

/// HSV を 0..=255 の RGB に変換する。h は内部で 0..=360 に正規化。
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)]
pub fn hsv_to_rgb(hsv: Hsv) -> (u8, u8, u8) {
    let h = hsv.h.rem_euclid(360.0);
    let s = hsv.s.clamp(0.0, 1.0);
    let v = hsv.v.clamp(0.0, 1.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to_u8 = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to_u8(r1), to_u8(g1), to_u8(b1))
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    #[test]
    fn grey_axis_has_zero_saturation() {
        // delta=0 のとき s = 0.0 が厳密に返る (rgb_to_hsv の実装より) ため float_cmp で許容。
        for v in [0u8, 32, 128, 200, 255] {
            let hsv = rgb_to_hsv(v, v, v);
            assert_eq!(hsv.s, 0.0, "grey at {v}");
            assert!((hsv.v - f32::from(v) / 255.0).abs() < 1e-6);
        }
    }

    #[test]
    fn pure_hues_map_to_expected_angles() {
        assert!((rgb_to_hsv(255, 0, 0).h - 0.0).abs() < 1e-3);
        assert!((rgb_to_hsv(0, 255, 0).h - 120.0).abs() < 1e-3);
        assert!((rgb_to_hsv(0, 0, 255).h - 240.0).abs() < 1e-3);
    }

    #[test]
    fn round_trip_within_one_unit() {
        // 各チャンネルを 16 ステップでサンプリングして round-trip 誤差を確認。
        for r in (0u8..=255).step_by(17) {
            for g in (0u8..=255).step_by(17) {
                for b in (0u8..=255).step_by(17) {
                    let (r2, g2, b2) = hsv_to_rgb(rgb_to_hsv(r, g, b));
                    assert!(r2.abs_diff(r) <= 1, "r: {r} -> {r2}");
                    assert!(g2.abs_diff(g) <= 1, "g: {g} -> {g2}");
                    assert!(b2.abs_diff(b) <= 1, "b: {b} -> {b2}");
                }
            }
        }
    }
}
