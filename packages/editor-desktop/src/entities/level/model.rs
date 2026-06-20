use serde::{Deserialize, Serialize};

/// 既定の base レイヤー画像名（`workspace/data/levels/{name}/base.png`）。
const DEFAULT_BASE: &str = "base.png";

/// 既定 Area。Z は画像ピクセル Y 相当 (near=手前=大, far=奥=小) なので near_z >= far_z。
/// X 範囲は near / far ともに 0..=640 の矩形相当。
const DEFAULT_NEAR_Z: i32 = 80;
const DEFAULT_FAR_Z: i32 = 0;
const DEFAULT_MIN_X: i32 = 0;
const DEFAULT_MAX_X: i32 = 640;

/// Camera 開始位置 / Player Spawn / Player Respawn Y の既定値はすべて 0。
const DEFAULT_ZERO: i32 = 0;

/// 1 つのゲームレベル (旧 Stage)。`workspace/data/levels/{name}.yml` に永続化される。
///
/// `name` はファイル名と二重管理を避けるため YAML には書かず、repository が注入する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Level {
    /// ファイル名から決まる識別子。serialize/deserialize 時はスキップする。
    #[serde(skip)]
    pub name: String,
    #[serde(default = "default_base")]
    pub base: String,
    /// base 画像の natural size (PNG header から読む)。loader が埋め、area / spawn の
    /// 画像内 clamp に使う。YAML には書かない。読めない (PNG 以外 / 不在) ときは None。
    #[serde(skip)]
    pub base_dimensions: Option<[u32; 2]>,
    /// 移動可能領域のリスト。複数指定された場合は OR 合成 (どれかに入っていれば移動可) の意味。
    /// YAML で空 / 未指定の場合は `[Area::default()]` 相当を補う。
    #[serde(default = "default_areas")]
    pub areas: Vec<Area>,

    /// Level 開始時にカメラがフォローしている world X (= 視界左端の画像 X)。
    #[serde(default = "default_zero")]
    pub camera_start_x: i32,
    /// Level 開始時のカメラ視界上端の画像 Y。
    #[serde(default = "default_zero")]
    pub camera_start_y: i32,

    /// Player の初期 spawn world X (Y は常に 0)。
    #[serde(default = "default_zero")]
    pub player_spawn_x: i32,
    /// Player の初期 spawn world Z。
    #[serde(default = "default_zero")]
    pub player_spawn_z: i32,

    /// Player 死亡時に再 spawn を始める world Y (落下開始 Y)。
    /// 0 で「地面 (Y=0) で即復活」、正の値で「上空から落下」。
    #[serde(default = "default_zero")]
    pub player_respawn_y: i32,

    /// Opponent (敵) の出現トリガー一覧。Player の world X が `trigger_x` に到達した瞬間に
    /// `(spawn_x, spawn_y, spawn_z)` へ `character_name` の Character を生成する。
    /// 空のとき YAML から省略する。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opponent_triggers: Vec<OpponentTrigger>,

    /// Level ごとの重力倍率。実効 gravity = `Character.physics.gravity * gravity_scale`。
    /// 月面ステージ (0.3) / 水中 (0.5) / 高重力 (2.0) などの演出差を作る用途。
    /// 省略時は通常重力 (= 1.0 相当) として扱う。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity_scale: Option<f32>,
}

/// XZ 平面上の移動可能領域。
///
/// 上下 2 辺 (Z = `near_z` / `far_z`) は常にスクリーン水平に平行で、左右の辺だけ斜めにできる
/// 1 辺平行台形。Z は画像ピクセル Y 相当で near=手前=画像下 (大)、far=奥=画像上 (小) なので
/// `near_z >= far_z`。`near_min_x == far_min_x` かつ `near_max_x == far_max_x` のとき矩形に縮退する。
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

/// Opponent (敵) の出現トリガー。
///
/// Player の world X が `trigger_x` に到達した瞬間に発火する 1-shot 条件 (engine 側責務)。
/// 1 個の trigger につき 1 体生成する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OpponentTrigger {
    /// 生成する Character の名前 (Character pool の identifier)。
    /// 文字列で参照する: rename / delete は editor 側で warning 表示のみで自動修正しない。
    #[serde(default)]
    pub character_name: String,
    /// Player の world X がこの値以上になった瞬間に発火する閾値。
    #[serde(default)]
    pub trigger_x: i32,
    /// 生成位置の world X。
    #[serde(default)]
    pub spawn_x: i32,
    /// 生成位置の world Y (高さ)。
    #[serde(default)]
    pub spawn_y: i32,
    /// 生成位置の world Z (奥行き)。
    #[serde(default)]
    pub spawn_z: i32,
}

impl Level {
    /// 名前付きの Default Level を返す。YAML が無い / 値未指定のときの fallback。
    #[must_use]
    pub fn with_defaults(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            base: DEFAULT_BASE.to_string(),
            base_dimensions: None,
            areas: default_areas(),
            camera_start_x: DEFAULT_ZERO,
            camera_start_y: DEFAULT_ZERO,
            player_spawn_x: DEFAULT_ZERO,
            player_spawn_z: DEFAULT_ZERO,
            player_respawn_y: DEFAULT_ZERO,
            opponent_triggers: Vec::new(),
            gravity_scale: None,
        }
    }
}

fn default_base() -> String {
    DEFAULT_BASE.to_string()
}

fn default_areas() -> Vec<Area> {
    vec![Area::default()]
}

fn default_zero() -> i32 {
    DEFAULT_ZERO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_with_defaults_has_empty_triggers_and_zero_spawn() {
        let lvl = Level::with_defaults("x");
        assert!(lvl.opponent_triggers.is_empty());
        assert_eq!(lvl.camera_start_x, 0);
        assert_eq!(lvl.camera_start_y, 0);
        assert_eq!(lvl.player_spawn_x, 0);
        assert_eq!(lvl.player_spawn_z, 0);
        assert_eq!(lvl.player_respawn_y, 0);
    }
}
