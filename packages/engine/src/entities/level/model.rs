//! Level の型定義 (FSD: model segment)。
//!
//! `runtime/data/levels/{name}.yml` の構造に対応する。editor 側の Level モデルから
//! engine 実行時に必要なフィールドだけを写し取った形。座標系は ADR-0023 に従い、
//! base 画像ピクセル = world (X, Z) で扱う。
//! ロード処理は隣の [`super::api`] に分離している。
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

const DEFAULT_BASE: &str = "base.png";

/// 既定 Area。Z は画像ピクセル Y 相当 (near=手前=大, far=奥=小) なので near_z >= far_z。
const DEFAULT_NEAR_Z: i32 = 80;
const DEFAULT_FAR_Z: i32 = 0;
const DEFAULT_MIN_X: i32 = 0;
const DEFAULT_MAX_X: i32 = 640;

/// 1 つのゲームレベル。`runtime/data/levels/{name}.yml` に永続化される。
///
/// `name` はファイル名と二重管理を避けるため YAML には書かず、loader が注入する。
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Level {
    #[serde(skip)]
    pub name: String,
    #[serde(default = "default_base")]
    pub base: String,
    /// 移動可能領域のリスト。複数指定された場合は OR 合成 (どれかに入っていれば移動可)。
    /// YAML で空 / 未指定の場合は `[Area::default()]` 相当を補う。
    #[serde(default = "default_areas")]
    pub areas: Vec<Area>,

    /// Level 開始時にカメラがフォローしている world X (= 視界左上隅の画像 X)。ADR-0023。
    #[serde(default)]
    pub camera_start_x: i32,
    /// Level 開始時のカメラ視界左上隅の画像 Y。
    #[serde(default)]
    pub camera_start_y: i32,

    /// Player の初期 spawn world X (Y は常に 0 = 地面)。
    #[serde(default)]
    pub player_spawn_x: i32,
    /// Player の初期 spawn world Z。
    #[serde(default)]
    pub player_spawn_z: i32,

    /// Player 死亡時に再 spawn を始める world Y (落下開始 Y)。0 で地面で即復活。
    #[serde(default)]
    pub player_respawn_y: i32,

    /// Opponent (敵) の出現トリガー一覧。Player の world X が `trigger_x` に到達した瞬間に
    /// `(spawn_x, spawn_y, spawn_z)` へ `character_name` の Character を生成する。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opponent_triggers: Vec<OpponentTrigger>,

    /// Level ごとの重力倍率。実効 gravity = `Character.physics.gravity * gravity_scale`。
    /// 省略時は通常重力 (= 1.0 相当)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity_scale: Option<f32>,
}

impl Default for Level {
    fn default() -> Self {
        Self::with_defaults("")
    }
}

impl Level {
    /// 名前付きの Default Level を返す。YAML が無い / 値未指定のときの fallback。
    #[must_use]
    pub fn with_defaults(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            base: DEFAULT_BASE.to_string(),
            areas: default_areas(),
            camera_start_x: 0,
            camera_start_y: 0,
            player_spawn_x: 0,
            player_spawn_z: 0,
            player_respawn_y: 0,
            opponent_triggers: Vec::new(),
            gravity_scale: None,
        }
    }

    /// 移動可能領域 (`areas`) のいずれかに点が入っていれば true。
    /// 空リストは制限なし扱い (ADR-0022 の fail-soft)。
    #[must_use]
    pub fn contains_xz(&self, x: f32, z: f32) -> bool {
        if self.areas.is_empty() {
            return true;
        }
        self.areas.iter().any(|a| a.contains_xz(x, z))
    }
}

/// XZ 平面上の移動可能領域 (ADR-0022)。
///
/// 上下 2 辺 (Z = `near_z` / `far_z`) は常にスクリーン水平に平行で、左右の辺だけ斜めにできる
/// 1 辺平行台形。Z は画像ピクセル Y 相当で near=手前=画像下 (大)、far=奥=画像上 (小) なので
/// `near_z >= far_z`。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Area {
    pub near_z: i32,
    pub far_z: i32,
    pub near_min_x: i32,
    pub near_max_x: i32,
    pub far_min_x: i32,
    pub far_max_x: i32,
}

impl Default for Area {
    fn default() -> Self {
        Self {
            near_z: DEFAULT_NEAR_Z,
            far_z: DEFAULT_FAR_Z,
            near_min_x: DEFAULT_MIN_X,
            near_max_x: DEFAULT_MAX_X,
            far_min_x: DEFAULT_MIN_X,
            far_max_x: DEFAULT_MAX_X,
        }
    }
}

impl Area {
    /// XZ 平面上の点がこの 1 辺平行台形に含まれるか (ADR-0022)。境界は含む。
    /// `near_z == far_z` (退化した線分) のときは `[near_min_x, near_max_x]` を使う。
    #[must_use]
    pub fn contains_xz(&self, x: f32, z: f32) -> bool {
        let nz = self.near_z as f32;
        let fz = self.far_z as f32;
        if z < fz || z > nz {
            return false;
        }
        let (left, right) = if self.near_z == self.far_z {
            (self.near_min_x as f32, self.near_max_x as f32)
        } else {
            let t = (z - nz) / (fz - nz);
            let left =
                self.near_min_x as f32 + t * (self.far_min_x - self.near_min_x) as f32;
            let right =
                self.near_max_x as f32 + t * (self.far_max_x - self.near_max_x) as f32;
            (left, right)
        };
        x >= left && x <= right
    }
}

/// Opponent の出現トリガー。
///
/// Player の world X が `trigger_x` に到達した瞬間に発火する 1-shot 条件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OpponentTrigger {
    #[serde(default)]
    pub character_name: String,
    #[serde(default)]
    pub trigger_x: i32,
    #[serde(default)]
    pub spawn_x: i32,
    #[serde(default)]
    pub spawn_y: i32,
    #[serde(default)]
    pub spawn_z: i32,
}

fn default_base() -> String {
    DEFAULT_BASE.to_string()
}

fn default_areas() -> Vec<Area> {
    vec![Area::default()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_with_defaults_has_empty_triggers_and_zero_spawn() {
        let lvl = Level::with_defaults("x");
        assert_eq!(lvl.name, "x");
        assert_eq!(lvl.base, "base.png");
        assert!(lvl.opponent_triggers.is_empty());
        assert_eq!(lvl.camera_start_x, 0);
        assert_eq!(lvl.camera_start_y, 0);
        assert_eq!(lvl.player_spawn_x, 0);
        assert_eq!(lvl.player_spawn_z, 0);
        assert_eq!(lvl.player_respawn_y, 0);
        assert!(lvl.gravity_scale.is_none());
    }

    #[test]
    fn area_default_preserves_near_z_ge_far_z_invariant() {
        // ADR-0023: near=手前=画像下=大, far=奥=画像上=小
        let a = Area::default();
        assert!(a.near_z >= a.far_z, "near_z must be >= far_z (ADR-0023)");
    }

    #[test]
    fn default_areas_is_single_area_with_default() {
        let areas = default_areas();
        assert_eq!(areas.len(), 1);
        assert_eq!(areas[0], Area::default());
    }

    #[test]
    fn area_contains_xz_inside_rectangle_includes_borders() {
        // Default Area: near_z=80, far_z=0, x: 0..=640
        let a = Area::default();
        assert!(a.contains_xz(100.0, 40.0));
        assert!(a.contains_xz(0.0, 0.0));
        assert!(a.contains_xz(640.0, 80.0));
    }

    #[test]
    fn area_contains_xz_rejects_outside_z() {
        let a = Area::default();
        assert!(!a.contains_xz(100.0, -0.5));
        assert!(!a.contains_xz(100.0, 80.5));
    }

    #[test]
    fn area_contains_xz_rejects_outside_x() {
        let a = Area::default();
        assert!(!a.contains_xz(-0.5, 40.0));
        assert!(!a.contains_xz(640.5, 40.0));
    }

    #[test]
    fn area_contains_xz_trapezoid_interpolates_left_right() {
        // 奥 (far) が狭い 台形。z=50 は near_z=100 と far_z=0 の中点なので
        // leftX = 0 + 0.5*(30-0) = 15, rightX = 100 + 0.5*(70-100) = 85
        let a = Area {
            near_z: 100,
            far_z: 0,
            near_min_x: 0,
            near_max_x: 100,
            far_min_x: 30,
            far_max_x: 70,
        };
        assert!(a.contains_xz(15.0, 50.0));
        assert!(a.contains_xz(85.0, 50.0));
        assert!(!a.contains_xz(14.0, 50.0));
        assert!(!a.contains_xz(86.0, 50.0));
        // far 側端 (z=0) は 30..=70 のみ
        assert!(a.contains_xz(30.0, 0.0));
        assert!(!a.contains_xz(29.0, 0.0));
    }

    #[test]
    fn area_contains_xz_handles_degenerate_z_equal() {
        // near_z == far_z は z=80 の線分扱い
        let a = Area {
            near_z: 80,
            far_z: 80,
            near_min_x: 0,
            near_max_x: 100,
            far_min_x: 50,
            far_max_x: 60,
        };
        assert!(a.contains_xz(0.0, 80.0));
        assert!(a.contains_xz(100.0, 80.0));
        assert!(!a.contains_xz(50.0, 79.9));
        assert!(!a.contains_xz(50.0, 80.1));
    }

    #[test]
    fn level_contains_xz_empty_areas_is_unrestricted() {
        let mut lvl = Level::with_defaults("x");
        lvl.areas.clear();
        assert!(lvl.contains_xz(-1_000_000.0, -1_000_000.0));
        assert!(lvl.contains_xz(1_000_000.0, 1_000_000.0));
    }

    #[test]
    fn level_contains_xz_composes_areas_with_or() {
        let mut lvl = Level::with_defaults("x");
        lvl.areas = vec![
            Area {
                near_z: 50,
                far_z: 0,
                near_min_x: 0,
                near_max_x: 100,
                far_min_x: 0,
                far_max_x: 100,
            },
            Area {
                near_z: 200,
                far_z: 150,
                near_min_x: 0,
                near_max_x: 100,
                far_min_x: 0,
                far_max_x: 100,
            },
        ];
        assert!(lvl.contains_xz(50.0, 25.0));
        assert!(lvl.contains_xz(50.0, 175.0));
        // gap (z=100) はどちらの area にも入らない
        assert!(!lvl.contains_xz(50.0, 100.0));
    }
}
