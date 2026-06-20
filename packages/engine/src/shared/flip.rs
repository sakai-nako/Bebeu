//! Sprite / Animation / Layer の反転モード。
//!
//! YAML 表現は `flip: horizontal` / `flip: vertical` / `flip: both`、未指定は `null`
//! (= flip なし)。複数レベルの flip (frame と layer) を合成するときは XOR で扱う:
//! 両方 flip すれば打ち消し合って元に戻る。
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlipMode {
    Horizontal,
    Vertical,
    Both,
}

impl FlipMode {
    #[must_use]
    pub fn flips_x(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    #[must_use]
    pub fn flips_y(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

/// `None` を「flip なし」として x 反転フラグを返す。
#[must_use]
pub fn flip_x_of(opt: Option<FlipMode>) -> bool {
    opt.is_some_and(FlipMode::flips_x)
}

/// `None` を「flip なし」として y 反転フラグを返す。
#[must_use]
pub fn flip_y_of(opt: Option<FlipMode>) -> bool {
    opt.is_some_and(FlipMode::flips_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flips_x_matrix() {
        assert!(FlipMode::Horizontal.flips_x());
        assert!(!FlipMode::Vertical.flips_x());
        assert!(FlipMode::Both.flips_x());
    }

    #[test]
    fn flips_y_matrix() {
        assert!(!FlipMode::Horizontal.flips_y());
        assert!(FlipMode::Vertical.flips_y());
        assert!(FlipMode::Both.flips_y());
    }

    #[test]
    fn flip_x_of_none_is_false() {
        assert!(!flip_x_of(None));
    }

    #[test]
    fn flip_x_of_horizontal_is_true() {
        assert!(flip_x_of(Some(FlipMode::Horizontal)));
        assert!(flip_x_of(Some(FlipMode::Both)));
        assert!(!flip_x_of(Some(FlipMode::Vertical)));
    }
}
