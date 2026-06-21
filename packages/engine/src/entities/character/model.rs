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

/// Animation の役割。engine 側 State (Idle/Walk/Attack/Hit/Jump/Block + 吹っ飛び flow) と
/// semantic に紐付ける。役割なしの YAML や Custom Animation は [`Role::Custom`] として扱う。
///
/// Knockback 系 (7 個 × 4 軸 = 通常 / Back / Dead / DeadBack の prefix; Rise は Dead 系 2 つを
/// 持たない) は ADR-0024/0025 の吹っ飛びフローに対応する。Animation 解決は
/// [`super::super::super::features::character::state_machine`] の `resolve_animation_role` が
/// `(state, hit_from_behind, final_action)` から 4 段フォールバック chain を試行する。
///
/// 旧 `dead` role は [`Role::DeadLieDown`] に集約 (serde alias で旧 YAML 互換)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Idle,
    Walk,
    Attack,
    Hit,
    Jump,
    Block,
    KnockbackUp,
    KnockbackDown,
    BounceUp,
    BounceDown,
    Slide,
    LieDown,
    Rise,
    BackKnockbackUp,
    BackKnockbackDown,
    BackBounceUp,
    BackBounceDown,
    BackSlide,
    BackLieDown,
    BackRise,
    DeadKnockbackUp,
    DeadKnockbackDown,
    DeadBounceUp,
    DeadBounceDown,
    DeadSlide,
    /// 死亡時の最終静止。Rise に進まず Animation 末尾で永続停止する。
    /// 旧形式 (`role: dead`) は alias でこの variant に読み替えられる。
    #[serde(alias = "dead")]
    DeadLieDown,
    DeadBackKnockbackUp,
    DeadBackKnockbackDown,
    DeadBackBounceUp,
    DeadBackBounceDown,
    DeadBackSlide,
    DeadBackLieDown,
    #[default]
    Custom,
}

impl Role {
    /// battle scene が起動時にロードを試みる Role 一覧 (Custom 以外)。順序は安定性のため
    /// 「基本 → Knockback (通常) → Back → Dead → DeadBack」で固定する。
    #[must_use]
    pub const fn all_loadable() -> &'static [Role] {
        &[
            Role::Idle,
            Role::Walk,
            Role::Attack,
            Role::Hit,
            Role::Jump,
            Role::Block,
            Role::KnockbackUp,
            Role::KnockbackDown,
            Role::BounceUp,
            Role::BounceDown,
            Role::Slide,
            Role::LieDown,
            Role::Rise,
            Role::BackKnockbackUp,
            Role::BackKnockbackDown,
            Role::BackBounceUp,
            Role::BackBounceDown,
            Role::BackSlide,
            Role::BackLieDown,
            Role::BackRise,
            Role::DeadKnockbackUp,
            Role::DeadKnockbackDown,
            Role::DeadBounceUp,
            Role::DeadBounceDown,
            Role::DeadSlide,
            Role::DeadLieDown,
            Role::DeadBackKnockbackUp,
            Role::DeadBackKnockbackDown,
            Role::DeadBackBounceUp,
            Role::DeadBackBounceDown,
            Role::DeadBackSlide,
            Role::DeadBackLieDown,
        ]
    }
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
    /// 被弾判定 box の **メイン**。Frame.body_box_overrides が None (Inherit) のときは
    /// この値が使われる。editor 側の Sprite.body_boxes と同じ schema。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_boxes: Option<Vec<HitBox>>,
    /// 攻撃判定 box の **メイン**。Frame.attack_box_overrides が None (Inherit) のときは
    /// この値が使われる。editor 側の Sprite.attack_boxes と同じ schema。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_boxes: Option<Vec<AttackBox>>,
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
    /// 60Hz vsync tick (= 1/60 秒) 単位。例: `7` で 7 tick = 約 116.67 ms。
    /// 0 のときは Default フレーム長 (engine 側で別途決める) を使うことを想定。
    /// **データモデル上の唯一の時間単位**で、engine / editor / animation export
    /// 全てがこの tick を 1 級概念として扱う (ms 概念は持ち込まない)。
    #[serde(default)]
    pub ticks: u32,
    /// frame レベルの反転 (frame 全体を反転)。`null` で反転なし。
    #[serde(default)]
    pub flip: Option<FlipMode>,
    /// `null` のとき (0, 0)。`Some([dx, dy])` のとき `dx`, `dy` が pivot に加算される。
    #[serde(default)]
    pub pivot_point_offset: Option<[i32; 2]>,
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// この frame で active な攻撃判定 box の上書き列。editor 側の 3-state と互換:
    /// `None`=Inherit (上位 SpriteEntry.attack_boxes に従う)、`Some(empty)`=Disable
    /// (攻撃判定なし)、`Some(non-empty)`=Override (この frame の box を使う)。
    /// 各 [`AttackBoxOverride`] 要素は hitbox / meta を個別に Option で持ち、None の field は
    /// sprite 側の同じ index の要素から継承する (`battle::resolve_attack_box` で merge)。
    #[serde(default)]
    pub attack_box_overrides: Option<Vec<AttackBoxOverride>>,
    /// この frame で active な被弾判定 box の上書き列。`attack_box_overrides` と同じ
    /// 3-state で、`None`=Inherit (`SpriteEntry.body_boxes` を継承)。
    #[serde(default)]
    pub body_box_overrides: Option<Vec<HitBox>>,
}

// === AttackBox / HitBox / AttackBoxMeta / KnockbackVec ===
// editor 側 (packages/editor-desktop/src/shared/collision.rs) と YAML 上互換になるよう
// フィールド名・形を揃える。engine は読み取り専用なので resize 系 helper は持たない。

/// `AttackBoxMeta.knockback` が保持する吹っ飛び速度ベクトル。
/// `vel_x` の符号は「攻撃側の前方向 = +」(scene 側で `Facing` を見て符号反転する)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct KnockbackVec {
    #[serde(default)]
    pub vel_x: f32,
    #[serde(default)]
    pub vel_y: f32,
    #[serde(default)]
    pub vel_z: f32,
}

/// 攻撃の効果データ。geometry (HitBox) と分離して、ダメージ / Knockback ゲージ削り /
/// 吹っ飛びベクトル / hit_stop 演出を表す (ADR-0024)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AttackBoxMeta {
    pub damage: u32,
    pub knockback_damage: u32,
    pub knockback: KnockbackVec,
    /// hit が決まった瞬間に発生する time freeze + sprite 揺らし演出。`None` で hit_stop なし
    /// (= 即座に通常の Hit state へ遷移)。詳細は [`HitStop`]。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hit_stop: Option<HitStop>,
}

/// hit 瞬間の time freeze + visual shake 演出パラメータ。攻撃側 frame の meta に置く
/// (= 攻撃の重さ・性質で決まる)。被弾側 sprite には三角波の shake が visual offset と
/// して乗り、その間 attacker / victim 両方の Animation 進行と CharacterState 遷移が
/// freeze される。world position は不動。
///
/// 軸の取り方:
/// - `shake_x`: キャラ向きの **前方** が +、後方が - (world X)。1 片道目の方向。
/// - `shake_y`: 画面上が +、画面下が - (world Y)。1 片道目の方向。
///
/// 三角波: `count` = 片道回数 (= 中心 ↔ ±max を 1 と数える)。1 = 中心 → +max で終了
/// (= 旧 impact 単発相当)、2 = 中心 → +max → 中心、4 = 1 周期 (中心 → +max → 中心 →
/// -max → 中心)。`decay` で振幅を線形に減衰させる。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HitStop {
    /// hit_stop の継続時間 (ms)。`None` のときは被弾側 Hit アニメ frame 0 の duration が
    /// そのまま使われる (= 被弾側固有値、ザコ vs ボス で揺れ時間を変えたい場合用の fallback)。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    /// shake の初期振幅 (px)。三角波の中心は 0、振幅は ±shake_x / ±shake_y。
    pub shake_x: i32,
    pub shake_y: i32,
    /// 片道回数。0 で shake なし。
    pub count: u32,
    /// shake 振幅の線形減衰率。`amplitude(progress) = shake * (1 - decay * progress).clamp(0, 1)`。
    /// 0.0 で振幅一定、1.0 で末尾の振幅 0。
    pub decay: f32,
}

/// 画像 pixel 座標で表された矩形 + 奥行き厚み (world Z)。
/// `top_left` / `bottom_right` は sprite 画像内ローカル座標、`depth` は world Z の全幅。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitBox {
    pub top_left: [i32; 2],
    pub bottom_right: [i32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

/// AttackBox = HitBox (幾何) + AttackBoxMeta (効果)。editor が serialize する新形式
/// (`{ hitbox: {...}, meta: {...} }`) を読む。旧形式 (HitBox 直接) は editor 側で
/// 新形式に migrate される前提で、engine 側では新形式のみ受ける。
/// sprite 側 (`SpriteEntry.attack_boxes`) のソース。`hitbox` は必須、`meta` のみ optional。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttackBox {
    pub hitbox: HitBox,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<AttackBoxMeta>,
}

/// `Frame.attack_box_overrides` の各要素。`hitbox` / `meta` を個別に Option で持ち、None の
/// field は sprite 側 (`SpriteEntry.attack_boxes`) の同じ index の要素から継承する。
/// 両方 Some なら sprite を完全に上書き、両方 None なら何もしない (= sprite をそのまま使う)。
/// YAML 互換: 既存 editor が書く `{ hitbox: {...}, meta: {...} }` の形 (両方 Some) は
/// そのまま deserialize できる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AttackBoxOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hitbox: Option<HitBox>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<AttackBoxMeta>,
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

    #[test]
    fn hit_stop_default_is_zero_offsets() {
        let hs = HitStop::default();
        assert_eq!(hs.duration_ms, None);
        assert_eq!(hs.shake_x, 0);
        assert_eq!(hs.shake_y, 0);
        assert_eq!(hs.count, 0);
        assert!((hs.decay - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn attack_box_meta_with_hit_stop_round_trip() -> anyhow::Result<()> {
        let yaml = r"
damage: 30
hit_stop:
  duration_ms: 120
  shake_x: 2
  shake_y: 4
  count: 3
  decay: 0.5
";
        let meta: AttackBoxMeta = serde_saphyr::from_str(yaml)?;
        assert_eq!(meta.damage, 30);
        let hs = meta.hit_stop.expect("hit_stop should be present");
        assert_eq!(hs.duration_ms, Some(120));
        assert_eq!(hs.shake_x, 2);
        assert_eq!(hs.shake_y, 4);
        assert_eq!(hs.count, 3);
        assert!((hs.decay - 0.5).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn attack_box_meta_without_hit_stop_field_yields_none() -> anyhow::Result<()> {
        // 既存 YAML (hit_stop なし) は meta.hit_stop = None で互換的に読める。
        let yaml = r"
damage: 30
knockback_damage: 5
";
        let meta: AttackBoxMeta = serde_saphyr::from_str(yaml)?;
        assert_eq!(meta.damage, 30);
        assert!(meta.hit_stop.is_none());
        Ok(())
    }

    #[test]
    fn hit_stop_without_duration_ms_field_defaults_to_none() -> anyhow::Result<()> {
        // duration_ms 省略時は被弾側 Hit アニメ frame 0 duration にフォールバックさせるため None。
        let yaml = r"
shake_x: 1
count: 2
";
        let hs: HitStop = serde_saphyr::from_str(yaml)?;
        assert_eq!(hs.duration_ms, None);
        assert_eq!(hs.shake_x, 1);
        assert_eq!(hs.count, 2);
        Ok(())
    }

    #[test]
    fn attack_box_round_trip_with_meta() -> anyhow::Result<()> {
        let yaml = r"
hitbox:
  top_left: [10, 20]
  bottom_right: [30, 40]
  depth: 12
meta:
  damage: 40
  knockback_damage: 30
  knockback:
    vel_x: 120.0
    vel_y: 80.0
    vel_z: 0.0
";
        let ab: AttackBox = serde_saphyr::from_str(yaml)?;
        assert_eq!(ab.hitbox.top_left, [10, 20]);
        assert_eq!(ab.hitbox.bottom_right, [30, 40]);
        assert_eq!(ab.hitbox.depth, Some(12));
        let meta = ab.meta.expect("meta should be present");
        assert_eq!(meta.damage, 40);
        assert_eq!(meta.knockback_damage, 30);
        assert!((meta.knockback.vel_x - 120.0).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn attack_box_without_meta_deserializes() -> anyhow::Result<()> {
        let yaml = r"
hitbox:
  top_left: [0, 0]
  bottom_right: [10, 10]
";
        let ab: AttackBox = serde_saphyr::from_str(yaml)?;
        assert!(ab.meta.is_none());
        assert_eq!(ab.hitbox.depth, None);
        Ok(())
    }

    #[test]
    fn frame_attack_box_overrides_round_trip() -> anyhow::Result<()> {
        // 既存 editor が書く形 (hitbox / meta 両方あり) の YAML 互換性を維持する。
        let yaml = r"
index: 1
ticks: 10
attack_box_overrides:
- hitbox:
    top_left: [18, 28]
    bottom_right: [42, 48]
  meta:
    damage: 40
layers: []
";
        let frame: Frame = serde_saphyr::from_str(yaml)?;
        let overrides = frame
            .attack_box_overrides
            .as_ref()
            .expect("overrides should be present");
        assert_eq!(overrides.len(), 1);
        let hb = overrides[0]
            .hitbox
            .as_ref()
            .expect("hitbox should be present");
        assert_eq!(hb.top_left, [18, 28]);
        assert_eq!(
            overrides[0]
                .meta
                .as_ref()
                .expect("meta should be present")
                .damage,
            40
        );
        Ok(())
    }

    #[test]
    fn attack_box_override_hitbox_only_omits_meta() -> anyhow::Result<()> {
        // partial override: hitbox だけ書く (meta は sprite から継承される想定)。
        let yaml = r"
hitbox:
  top_left: [10, 20]
  bottom_right: [30, 40]
";
        let ov: AttackBoxOverride = serde_saphyr::from_str(yaml)?;
        assert!(ov.hitbox.is_some());
        assert!(ov.meta.is_none());
        Ok(())
    }

    #[test]
    fn attack_box_override_meta_only_omits_hitbox() -> anyhow::Result<()> {
        // partial override: meta だけ書く (hitbox は sprite から継承される想定)。
        let yaml = r"
meta:
  damage: 75
";
        let ov: AttackBoxOverride = serde_saphyr::from_str(yaml)?;
        assert!(ov.hitbox.is_none());
        assert_eq!(ov.meta.expect("meta should be present").damage, 75);
        Ok(())
    }

    #[test]
    fn attack_box_override_empty_object_is_noop() -> anyhow::Result<()> {
        // 両方 None: sprite を上書きしない (= sprite をそのまま使う)。
        let yaml = "{}";
        let ov: AttackBoxOverride = serde_saphyr::from_str(yaml)?;
        assert!(ov.hitbox.is_none());
        assert!(ov.meta.is_none());
        Ok(())
    }
}
