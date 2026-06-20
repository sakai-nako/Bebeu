//! Character 集約の全型定義 (FSD: model segment)。
//!
//! editor-desktop の同名スライスとは独立で、engine 描画に必要なフィールドのみを保持する。
//! editor 専用フィールド (`body_box_overrides` / `attack_box_overrides` / `sound` /
//! `export_number`) は serde の未知フィールドとして silently ignore される。
//! ロード処理は隣の [`super::api`] に分離している。
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::shared::flip::FlipMode;

// === Defaults ===

pub const DEFAULT_DEPTH: u32 = 16;
pub const DEFAULT_HP: u32 = 100;

pub const DEFAULT_GRAVITY: f64 = 800.0;
pub const DEFAULT_JUMP_VELOCITY_Y: f64 = 200.0;
pub const DEFAULT_KNOCKBACK_THRESHOLD: u32 = 100;
pub const DEFAULT_MAX_BOUNCE_COUNT: u32 = 1;
pub const DEFAULT_BOUNCE_DAMPENING: f32 = 0.5;
pub const DEFAULT_GROUND_FRICTION: f64 = 600.0;
pub const DEFAULT_HIT_RECOVERY_MS: u32 = 1500;
pub const DEFAULT_LIE_DOWN_DURATION_MS: u32 = 800;
pub const DEFAULT_RISE_DURATION_MS: u32 = 300;

// === Role ===

/// Animation の役割。engine 側 State (Idle/Walk/Attack/Hit/Dead/Jump/Block) と
/// semantic に紐付ける。役割なしの YAML や Custom Animation は [`Role::Custom`] として扱う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Idle,
    Walk,
    Attack,
    Hit,
    Dead,
    Jump,
    Block,
    #[default]
    Custom,
}

// === Physics ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Physics {
    #[serde(default)]
    pub gravity: f64,
    #[serde(default)]
    pub jump_velocity_y: f64,
    #[serde(default)]
    pub knockback_threshold: u32,
    #[serde(default)]
    pub knockback_resistance: f32,
    #[serde(default)]
    pub max_bounce_count: u32,
    #[serde(default)]
    pub bounce_dampening: f32,
    #[serde(default)]
    pub ground_friction: f64,
    #[serde(default)]
    pub hit_recovery_ms: u32,
    #[serde(default)]
    pub lie_down_duration_ms: u32,
    #[serde(default)]
    pub rise_duration_ms: u32,
}

impl Default for Physics {
    fn default() -> Self {
        Self {
            gravity: DEFAULT_GRAVITY,
            jump_velocity_y: DEFAULT_JUMP_VELOCITY_Y,
            knockback_threshold: DEFAULT_KNOCKBACK_THRESHOLD,
            knockback_resistance: 0.0,
            max_bounce_count: DEFAULT_MAX_BOUNCE_COUNT,
            bounce_dampening: DEFAULT_BOUNCE_DAMPENING,
            ground_friction: DEFAULT_GROUND_FRICTION,
            hit_recovery_ms: DEFAULT_HIT_RECOVERY_MS,
            lie_down_duration_ms: DEFAULT_LIE_DOWN_DURATION_MS,
            rise_duration_ms: DEFAULT_RISE_DURATION_MS,
        }
    }
}

// === Character ===

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Character {
    /// YAML に `name` が無ければ file stem を [`super::api`] 側で埋める。
    #[serde(default)]
    pub name: String,
    /// editor が生成する thumbnail への、character ディレクトリ起点の相対パス。
    /// 実体は `runtime/data/characters/{name}/{thumbnail_path}`。AssetServer 経由でロード可能。
    #[serde(default)]
    pub thumbnail_path: String,
    #[serde(default = "default_hp")]
    pub hp: u32,
    #[serde(default = "default_depth")]
    pub depth: u32,
    #[serde(default)]
    pub physics: Physics,
    /// `runtime/data/characters/{name}/sprite-groups/*.yml` から populate される。
    /// key は `SpriteGroup.number` (= Layer.sprite_group_number から参照)。
    /// YAML には書かれない。
    #[serde(skip)]
    pub sprite_groups: HashMap<u32, SpriteGroup>,
    /// `runtime/data/characters/{name}/animations/*.yml` から populate される。
    /// YAML には書かれない。
    #[serde(skip)]
    pub animations: Vec<Animation>,
}

// === SpriteGroup / SpriteEntry ===

/// `runtime/data/characters/{character}/sprite-groups/{group}.yml` の構造。
/// engine 描画では `pivot_point` だけが必須なので、body_boxes / attack_boxes は
/// serde の未知フィールドとして silently ignore される。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpriteGroup {
    /// YAML には書かれず、loader (api 側) が file stem で埋める。
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub sprites: Vec<SpriteEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpriteEntry {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub path: String,
    /// 画像内のキャラ pivot 位置 (ピクセル, [x, y])。
    /// Go 版 engine の `layer_origin = char_pos + (-sprite.pivot_point) + ...` で
    /// 「画像のここを char_pos に重ねる」基準点。
    #[serde(default)]
    pub pivot_point: [i32; 2],
}

// === Animation / Frame / Layer ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Animation {
    /// YAML には書かれず、loader (api 側) が file stem で埋める。
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub role: Role,
    /// Multi-cardinality role (Attack/Hit/Dead/Jump) の役割内 slot 番号 (0-indexed)。
    /// Single-cardinality role では 0 固定。
    #[serde(default)]
    pub variant: u32,
    #[serde(default)]
    pub is_loop: bool,
    #[serde(default)]
    pub loop_start_index: u32,
    #[serde(default)]
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Frame {
    #[serde(default)]
    pub index: u32,
    /// ms 単位。0 のときは Default フレーム長 (engine 側で別途決める) を使うことを想定。
    #[serde(default)]
    pub duration: u32,
    /// frame レベルの反転 (frame 全体を反転)。`null` で反転なし。
    #[serde(default)]
    pub flip: Option<FlipMode>,
    /// `null` のとき (0, 0)。`Some([dx, dy])` のとき `dx`, `dy` が pivot に加算される。
    #[serde(default)]
    pub pivot_point_offset: Option<[i32; 2]>,
    #[serde(default)]
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Layer {
    #[serde(default)]
    pub index: u32,
    /// 参照する `SpriteGroup` を **number** で指定する (name のリネーム耐性)。
    #[serde(default)]
    pub sprite_group_number: u32,
    /// SpriteGroup 内の Sprite を **index** で指定する (filename 変更耐性)。
    #[serde(default)]
    pub sprite_index: u32,
    /// 0.0 〜 1.0 の透明度。1.0 で完全不透明。
    #[serde(default = "default_transparency")]
    pub transparency: f32,
    /// layer レベルの反転 (frame 内でこの layer のみを反転)。`null` で反転なし。
    /// 最終 flip = facing XOR (frame.flip XOR layer.flip)。
    #[serde(default)]
    pub flip: Option<FlipMode>,
    #[serde(default)]
    pub pivot_point_offset: Option<[i32; 2]>,
}

impl Frame {
    /// `pivot_point_offset` を `(x, y)` で取り出す。`None` のときは (0, 0)。
    #[must_use]
    pub fn pivot_offset_xy(&self) -> (i32, i32) {
        offset_xy(self.pivot_point_offset)
    }
}

impl Layer {
    /// `pivot_point_offset` を `(x, y)` で取り出す。`None` のときは (0, 0)。
    #[must_use]
    pub fn pivot_offset_xy(&self) -> (i32, i32) {
        offset_xy(self.pivot_point_offset)
    }
}

// === helpers ===

fn default_hp() -> u32 {
    DEFAULT_HP
}

fn default_depth() -> u32 {
    DEFAULT_DEPTH
}

fn default_transparency() -> f32 {
    1.0
}

fn offset_xy(opt: Option<[i32; 2]>) -> (i32, i32) {
    opt.map_or((0, 0), |a| (a[0], a[1]))
}

// === tests ===

#[cfg(test)]
mod tests {
    use super::*;

    // Physics::default は DEFAULT_* 定数をそのまま代入しているので bit-exact 一致を期待する。
    #[test]
    #[allow(clippy::float_cmp)]
    fn physics_default_matches_engine_constants() {
        let p = Physics::default();
        assert_eq!(p.gravity, DEFAULT_GRAVITY);
        assert_eq!(p.jump_velocity_y, DEFAULT_JUMP_VELOCITY_Y);
        assert_eq!(p.knockback_threshold, DEFAULT_KNOCKBACK_THRESHOLD);
        assert_eq!(p.knockback_resistance, 0.0);
        assert_eq!(p.max_bounce_count, DEFAULT_MAX_BOUNCE_COUNT);
        assert_eq!(p.bounce_dampening, DEFAULT_BOUNCE_DAMPENING);
        assert_eq!(p.ground_friction, DEFAULT_GROUND_FRICTION);
        assert_eq!(p.hit_recovery_ms, DEFAULT_HIT_RECOVERY_MS);
        assert_eq!(p.lie_down_duration_ms, DEFAULT_LIE_DOWN_DURATION_MS);
        assert_eq!(p.rise_duration_ms, DEFAULT_RISE_DURATION_MS);
    }

    #[test]
    fn frame_pivot_offset_xy_none_returns_zero() {
        let f = Frame::default();
        assert_eq!(f.pivot_offset_xy(), (0, 0));
    }

    #[test]
    fn frame_pivot_offset_xy_some_returns_values() {
        let f = Frame {
            pivot_point_offset: Some([4, -7]),
            ..Frame::default()
        };
        assert_eq!(f.pivot_offset_xy(), (4, -7));
    }

    #[test]
    fn layer_pivot_offset_xy_none_returns_zero() {
        let l = Layer::default();
        assert_eq!(l.pivot_offset_xy(), (0, 0));
    }

    #[test]
    fn layer_default_transparency_is_zero_via_struct_default() {
        // Default::default() は #[serde(default = "default_transparency")] を経由しない
        // ので f32::default() = 0.0 になる点に注意。YAML 由来は 1.0 に倒れる
        // (test は YAML 経由のロード後で見る、ここでは struct default 値を文書化)。
        let l = Layer::default();
        assert!((l.transparency - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn animation_default_role_is_custom() {
        let a = Animation::default();
        assert_eq!(a.role, Role::Custom);
    }
}
