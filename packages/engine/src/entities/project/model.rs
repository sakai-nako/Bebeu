//! Project の型定義 (FSD: model segment)。
//!
//! `runtime/data/projects/{name}.yml` の構造に対応する。
//! ロード処理は隣の [`super::api`] に分離している。
use std::collections::HashMap;

use bevy::prelude::{Color, Resource};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::entities::character::Role;
use crate::shared::PlayerId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            width: 384,
            height: 216,
        }
    }
}

/// Project は battle 起動時に「どのキャラとどのレベルを使うか」を束ねた起点データ。
///
/// YAML 側にはファイル名 = `name` のため `name` フィールドは持たない。
/// ロード時にファイル stem を埋める。
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub resolution: Resolution,
    #[serde(default)]
    pub players: Vec<String>,
    #[serde(default)]
    pub opponents: Vec<String>,
    /// ADR-0035 Phase 2: 共闘する味方 NPC のキャラ名。battle 起動時に players の隣で
    /// spawn され、`AllyBrain` で driven される。未指定 / 空なら従来通り Player 単独。
    #[serde(default)]
    pub allies: Vec<String>,
    #[serde(default)]
    pub levels: Vec<String>,
    #[serde(default)]
    pub hud: Hud,
}

/// gameplay 中の HUD レイアウト (ADR-0029)。要素は配列で、project ごとに自由に並べる。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hud {
    #[serde(default)]
    pub elements: Vec<HudElement>,
}

/// HUD 1 要素。internally-tagged enum で `kind:` がそのまま YAML 上の判別キーになる
/// (ADR-0029)。kind を増やすときは variant を追加し、対応する Config struct を新設する。
///
/// `Copy` は外している (ADR-0031: id / tag が `String` を含む)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HudElement {
    PlayerHpBar(PlayerHpBarConfig),
    PlayerHpRing(PlayerHpRingConfig),
    EnemyHpBar(EnemyHpBarConfig),
    /// ADR-0032: world-anchored。spawn_hud の screen-anchor 経路は通らず、Enemy 各 entity に
    /// child として attach される。`anchor` / `anchor_to` / `offset` は意味を持たない。
    EnemyOverheadHpBar(EnemyOverheadHpBarConfig),
    /// ADR-0033: Player の CharacterState に応じて Icon を切り替える HUD 要素。
    /// Character.sprite_groups の `sprite_group_number` を指す 1 つの group の中で、
    /// 各 Role に対応する `sprite_index` を `state_sprites` で割り当てる。
    /// `shake` で被弾 / attack 当てを trigger に独自振動を入れられる (HitStopState 連動ではない)。
    PlayerIcon(PlayerIconConfig),
}

impl HudElement {
    /// この要素が screen-anchored か (= spawn_hud の通常経路で扱うか)。
    /// `false` の variant は world-anchored で、別 system が `Added<Enemy>` 等で per-entity spawn する。
    #[must_use]
    pub fn is_screen_anchored(&self) -> bool {
        match self {
            HudElement::PlayerHpBar(_)
            | HudElement::PlayerHpRing(_)
            | HudElement::EnemyHpBar(_)
            | HudElement::PlayerIcon(_) => true,
            HudElement::EnemyOverheadHpBar(_) => false,
        }
    }

    /// 描画位置の screen anchor を返す。`anchor_to.is_some()` のときはこの値は無視される。
    /// world-anchored の variant では呼ばないこと (default を返すだけ)。
    #[must_use]
    pub fn anchor(&self) -> HudAnchor {
        match self {
            HudElement::PlayerHpBar(c) => c.anchor,
            HudElement::PlayerHpRing(c) => c.anchor,
            HudElement::EnemyHpBar(c) => c.anchor,
            HudElement::PlayerIcon(c) => c.anchor,
            HudElement::EnemyOverheadHpBar(_) => HudAnchor::default(),
        }
    }

    /// `Some` のとき: 他要素を基準点に取る (ADR-0031)。`anchor` より優先される。
    #[must_use]
    pub fn anchor_to(&self) -> Option<&HudElementAnchor> {
        match self {
            HudElement::PlayerHpBar(c) => c.anchor_to.as_ref(),
            HudElement::PlayerHpRing(c) => c.anchor_to.as_ref(),
            HudElement::EnemyHpBar(c) => c.anchor_to.as_ref(),
            HudElement::PlayerIcon(c) => c.anchor_to.as_ref(),
            HudElement::EnemyOverheadHpBar(_) => None,
        }
    }

    /// 描画位置の offset。screen / element anchor のどちらでも有効。
    #[must_use]
    pub fn offset(&self) -> HudOffset {
        match self {
            HudElement::PlayerHpBar(c) => c.offset,
            HudElement::PlayerHpRing(c) => c.offset,
            HudElement::EnemyHpBar(c) => c.offset,
            HudElement::PlayerIcon(c) => c.offset,
            HudElement::EnemyOverheadHpBar(_) => HudOffset::default(),
        }
    }

    /// 他要素から `anchor_to.id` で参照されるための識別子 (ADR-0031)。
    /// `None` の要素は他の要素から参照できない (= 末端のみ)。
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match self {
            HudElement::PlayerHpBar(c) => c.id.as_deref(),
            HudElement::PlayerHpRing(c) => c.id.as_deref(),
            HudElement::EnemyHpBar(c) => c.id.as_deref(),
            HudElement::PlayerIcon(c) => c.id.as_deref(),
            HudElement::EnemyOverheadHpBar(_) => None,
        }
    }

    /// 描画サイズ (外形 bbox)。Element anchor の edge 計算に使う。
    #[must_use]
    pub fn size(&self) -> HudSize {
        match self {
            HudElement::PlayerHpBar(c) => c.size,
            HudElement::PlayerHpRing(c) => c.size,
            HudElement::EnemyHpBar(c) => c.size,
            HudElement::PlayerIcon(c) => c.size,
            HudElement::EnemyOverheadHpBar(c) => c.size,
        }
    }
}

/// 他 HUD 要素を基準点に取る anchor (ADR-0031)。`id` で要素を引き、`edge` で
/// その要素の外形 bbox 上の 9 隅のどこを基準点にするか指定する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudElementAnchor {
    /// 参照先 `HudElement.id`。
    pub id: String,
    /// 参照先要素の bbox 上の基準点 (9 隅、`HudAnchor` 型を流用)。
    #[serde(default)]
    pub edge: HudAnchor,
}

/// Player HP バーの表示設定。size は外形 bbox で、`frame.thickness` 分だけ
/// ゲージ描画領域が内側に縮む (= frame は size の内側に食い込む)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerHpBarConfig {
    /// ADR-0031: 他の HUD 要素から `anchor_to.id` で参照される識別子。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub target: PlayerId,
    #[serde(default)]
    pub anchor: HudAnchor,
    /// ADR-0031: `Some` のとき他要素を基準点に取る。`anchor` より優先される。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_to: Option<HudElementAnchor>,
    #[serde(default)]
    pub offset: HudOffset,
    pub size: HudSize,
    #[serde(default)]
    pub frame: HudFrame,
    #[serde(default = "default_hp_bar_bg_color")]
    pub bg_color: HexColor,
    #[serde(default = "default_hp_bar_fg_color")]
    pub fg_color: HexColor,
    #[serde(default)]
    pub fill_direction: FillDirection,
    #[serde(default)]
    pub gauge_step: GaugeStep,
    #[serde(default)]
    pub gauge_gap: f32,
}

impl Default for PlayerHpBarConfig {
    fn default() -> Self {
        Self {
            id: None,
            target: PlayerId::default(),
            anchor: HudAnchor::TopLeft,
            anchor_to: None,
            offset: HudOffset { x: 16.0, y: 16.0 },
            size: HudSize { w: 120.0, h: 8.0 },
            frame: HudFrame::default(),
            bg_color: default_hp_bar_bg_color(),
            fg_color: default_hp_bar_fg_color(),
            fill_direction: FillDirection::default(),
            gauge_step: GaugeStep::default(),
            gauge_gap: 0.0,
        }
    }
}

fn default_hp_bar_bg_color() -> HexColor {
    HexColor {
        r: 0,
        g: 0,
        b: 0,
        a: 153,
    }
}

fn default_hp_bar_fg_color() -> HexColor {
    HexColor {
        r: 229,
        g: 38,
        b: 38,
        a: 255,
    }
}

/// Player HP リング (annular sector) の表示設定。
///
/// `size` は外接 bbox。中心と半径は `min(w, h) / 2` で決まる。`ring_thickness` 分だけ
/// 外側から内側へ帯を描く (`= 0` で扇形になる)。`start_angle` は 12 時方向を 0° とし、
/// `direction` が `clockwise` のとき時計回りに `sweep_extent` 度ぶん描画する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerHpRingConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub target: PlayerId,
    #[serde(default)]
    pub anchor: HudAnchor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_to: Option<HudElementAnchor>,
    #[serde(default)]
    pub offset: HudOffset,
    pub size: HudSize,
    #[serde(default)]
    pub frame: HudFrame,
    #[serde(default = "default_hp_bar_bg_color")]
    pub bg_color: HexColor,
    #[serde(default = "default_hp_bar_fg_color")]
    pub fg_color: HexColor,
    #[serde(default = "default_ring_start_angle")]
    pub start_angle: f32,
    #[serde(default = "default_ring_sweep_extent")]
    pub sweep_extent: f32,
    #[serde(default = "default_ring_thickness")]
    pub ring_thickness: f32,
    #[serde(default)]
    pub direction: RingDirection,
    #[serde(default)]
    pub gauge_step: GaugeStep,
    #[serde(default)]
    pub gauge_gap: f32,
}

impl Default for PlayerHpRingConfig {
    fn default() -> Self {
        Self {
            id: None,
            target: PlayerId::default(),
            anchor: HudAnchor::TopLeft,
            anchor_to: None,
            offset: HudOffset { x: 16.0, y: 16.0 },
            size: HudSize { w: 48.0, h: 48.0 },
            frame: HudFrame::default(),
            bg_color: default_hp_bar_bg_color(),
            fg_color: default_hp_bar_fg_color(),
            start_angle: default_ring_start_angle(),
            sweep_extent: default_ring_sweep_extent(),
            ring_thickness: default_ring_thickness(),
            direction: RingDirection::default(),
            gauge_step: GaugeStep::default(),
            gauge_gap: 0.0,
        }
    }
}

/// Enemy の HP を映す HUD 要素 (ADR-0031)。形は `PlayerHpBarConfig` と同じだが、
/// `target` は `EnemyTarget` (engagement-link / tag / nth_enemy) で動的解決する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyHpBarConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub target: EnemyTarget,
    #[serde(default)]
    pub anchor: HudAnchor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_to: Option<HudElementAnchor>,
    #[serde(default)]
    pub offset: HudOffset,
    pub size: HudSize,
    #[serde(default)]
    pub frame: HudFrame,
    #[serde(default = "default_hp_bar_bg_color")]
    pub bg_color: HexColor,
    #[serde(default = "default_enemy_hp_bar_fg_color")]
    pub fg_color: HexColor,
    #[serde(default)]
    pub fill_direction: FillDirection,
    #[serde(default)]
    pub gauge_step: GaugeStep,
    #[serde(default)]
    pub gauge_gap: f32,
}

impl Default for EnemyHpBarConfig {
    fn default() -> Self {
        Self {
            id: None,
            target: EnemyTarget::default(),
            anchor: HudAnchor::Top,
            anchor_to: None,
            offset: HudOffset { x: 0.0, y: 16.0 },
            size: HudSize { w: 120.0, h: 8.0 },
            frame: HudFrame::default(),
            bg_color: default_hp_bar_bg_color(),
            fg_color: default_enemy_hp_bar_fg_color(),
            fill_direction: FillDirection::default(),
            gauge_step: GaugeStep::default(),
            gauge_gap: 0.0,
        }
    }
}

/// Enemy bar の default 色は Player bar (赤) と区別するため黄色寄り。
fn default_enemy_hp_bar_fg_color() -> HexColor {
    HexColor {
        r: 242,
        g: 216,
        b: 44,
        a: 255,
    }
}

/// HUD の `enemy_hp_bar` 要素がどの Enemy を映すか (ADR-0031)。
/// externally-tagged enum で YAML key がそのまま variant 名。
///
/// - `LastEngagedBy(PlayerId)`: 指定 Player が直近で attack を当てた enemy。Player の
///   `LastEngagedWith` を読んで resolve する。engagement-link 系の表示に使う。
/// - `Tag(String)`: Enemy character の `tag` field と一致する最初の entity。Boss 用。
/// - `NthEnemy(usize)`: 出現順 n 番目の Enemy。0-origin。debug 用途中心。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyTarget {
    LastEngagedBy(PlayerId),
    Tag(String),
    NthEnemy(usize),
}

impl Default for EnemyTarget {
    fn default() -> Self {
        Self::LastEngagedBy(PlayerId::P1)
    }
}

/// world-anchored な Enemy 頭上 HP bar の表示設定 (ADR-0032)。
///
/// `tag_filter` 省略時は **全 Enemy entity に 1 本ずつ** 自動で attach される。`Some` なら
/// `EnemyTag` が一致する Enemy にだけ attach される (= boss 専用 overhead 等)。
///
/// 位置は `vertical_anchor` + `offset_y` で決まり、毎 frame **現フレームの sprite 形状**
/// に追従して再計算される。default は `vertical_anchor: image_top` + `offset_y: 4` (= 画像
/// 上端から 4 px 上)、これでキャラ身長に依らず常に頭上の少し上に表示できる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyOverheadHpBarConfig {
    /// 一致する `EnemyTag` を持つ enemy にだけ attach する。省略時は全 enemy。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_filter: Option<String>,
    pub size: HudSize,
    #[serde(default)]
    pub frame: HudFrame,
    #[serde(default = "default_hp_bar_bg_color")]
    pub bg_color: HexColor,
    #[serde(default = "default_enemy_hp_bar_fg_color")]
    pub fg_color: HexColor,
    /// bar の Y 基準。`image_top` / `image_bottom` は **毎 frame** 現 sprite の形状に追従。
    #[serde(default)]
    pub vertical_anchor: OverheadVerticalAnchor,
    /// `vertical_anchor` からの bevy Y オフセット (px、+ が上、- が下)。
    /// default 4 (image_top から 4 px 上 = 頭の少し上)。
    #[serde(default = "default_overhead_offset_y")]
    pub offset_y: f32,
    /// 横方向 fill が主用途 (LeftToRight / RightToLeft)。Default = LeftToRight。
    #[serde(default)]
    pub fill_direction: FillDirection,
}

impl Default for EnemyOverheadHpBarConfig {
    fn default() -> Self {
        Self {
            tag_filter: None,
            size: HudSize { w: 28.0, h: 3.0 },
            frame: HudFrame::default(),
            bg_color: default_hp_bar_bg_color(),
            fg_color: default_enemy_hp_bar_fg_color(),
            vertical_anchor: OverheadVerticalAnchor::default(),
            offset_y: default_overhead_offset_y(),
            fill_direction: FillDirection::LeftToRight,
        }
    }
}

fn default_overhead_offset_y() -> f32 {
    4.0
}

/// Player の CharacterState に応じて Icon を切り替える HUD 要素 (ADR-0033)。
///
/// Character.sprite_groups の中から `sprite_group_number` で 1 つの group を選び、
/// `state_sprites` で各 Role に対応する sprite index を割り当てる。
/// state が `state_sprites` に未登録のときは `default_sprite_index` の sprite を表示する。
///
/// engine 側の CharacterState (= features 配下) を直接 key にできない (FSD: entities は
/// features を見ない) ため、`Role` (entities/character) を key にする。engine が
/// `CharacterState::to_role()` で変換して引く。
///
/// `shake` で被弾 (HP 減) / attack 当て (HitStopState::attacker attach) を trigger にした
/// 独自振動を仕込める。Player 本体の HitStopState とは独立 (= Icon HUD だけが揺れる)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerIconConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub target: PlayerId,
    #[serde(default)]
    pub anchor: HudAnchor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_to: Option<HudElementAnchor>,
    #[serde(default)]
    pub offset: HudOffset,
    pub size: HudSize,
    #[serde(default)]
    pub frame: HudFrame,
    #[serde(default = "default_icon_bg_color")]
    pub bg_color: HexColor,
    /// 対象 Player の Character.sprite_groups から引く group の number。
    /// Icon HUD 用に専用の sprite group を 1 つ用意して、そこに各 state 用の Image を集める。
    pub sprite_group_number: u32,
    /// state_sprites に該当しないときの fallback。group 内のどの sprite_index を表示するか。
    #[serde(default)]
    pub default_sprite_index: u32,
    /// Role → sprite_index の map。CharacterState は to_role() で Role に変換して引く。
    #[serde(default)]
    pub state_sprites: HashMap<Role, u32>,
    /// 振動 trigger と振動パラメータ。default は両 trigger 無効 = 振動なし。
    #[serde(default)]
    pub shake: IconShakeConfig,
}

impl Default for PlayerIconConfig {
    fn default() -> Self {
        Self {
            id: None,
            target: PlayerId::default(),
            anchor: HudAnchor::TopLeft,
            anchor_to: None,
            offset: HudOffset { x: 16.0, y: 16.0 },
            size: HudSize { w: 32.0, h: 32.0 },
            frame: HudFrame::default(),
            bg_color: default_icon_bg_color(),
            sprite_group_number: 0,
            default_sprite_index: 0,
            state_sprites: HashMap::new(),
            shake: IconShakeConfig::default(),
        }
    }
}

/// Icon HUD の default bg。完全透明 (= 枠だけ + 画像)。HP bar 系と違って
/// Icon は画像が前提なので背景は塗らない。
fn default_icon_bg_color() -> HexColor {
    HexColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    }
}

/// PlayerIcon の振動 trigger と振動パラメータ (ADR-0033)。
/// trigger を `Some` にしないと、その trigger では振動しない (default は両方 None)。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IconShakeConfig {
    /// Player の HitPoints が **減ったとき** に発火 (= 被弾 / ガード damage 等)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_damage: Option<IconShakeParams>,
    /// Player が attack を当てたとき (= attacker に HitStopState が attach されたとき) に発火。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_attack_hit: Option<IconShakeParams>,
}

/// 振動パラメータ。`HitStopState` と同じ三角波 + 線形減衰モデル。
/// 軸の取り方は HitStop と違って画面基準 (Facing 反転しない)。
/// + X = 画面右、+ Y = 画面上。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct IconShakeParams {
    /// 振動の継続時間 (ms)。
    #[serde(default)]
    pub duration_ms: u32,
    /// 振動の初期振幅 (px)。
    #[serde(default)]
    pub shake_x: i32,
    #[serde(default)]
    pub shake_y: i32,
    /// 三角波の片道回数。0 で振動なし (= duration だけ進んで終わる)。
    #[serde(default)]
    pub count: u32,
    /// 振幅の線形減衰率。0.0 で振幅一定、1.0 で末尾の振幅 0。
    #[serde(default)]
    pub decay: f32,
}

/// overhead bar の Y 基準 (ADR-0032 拡張)。
///
/// - `Origin`: Enemy entity の Transform 原点 (= sprite anchor、通常は足元) から `offset_y`
///   上に置く。キャラ身長に応じて offset_y を手動調整する必要がある。
/// - `ImageTop` (default): 現フレームの sprite 画像 **上端** から `offset_y` 上に置く。
///   キャラ姿勢で sprite 高さが変わっても自動で頭上に追従する (jump で sprite が縦に
///   伸びれば bar も上に動く)。
/// - `ImageBottom`: 現フレームの sprite 画像 **下端** から `offset_y` 上に置く。
///   sprite に shadow が伸びていて、その上端 + bar を出す用途。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverheadVerticalAnchor {
    Origin,
    #[default]
    ImageTop,
    ImageBottom,
}

fn default_ring_start_angle() -> f32 {
    0.0
}

fn default_ring_sweep_extent() -> f32 {
    360.0
}

fn default_ring_thickness() -> f32 {
    6.0
}

/// リングが描画される回転方向。`gauge_step` で複数 segment に分けたとき、終端側
/// (clockwise なら一番反時計側) の segment から HP 減少で消える。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RingDirection {
    #[default]
    Clockwise,
    CounterClockwise,
}

/// 画面上の基準点。anchor から offset 分だけずらした位置に要素を置く。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HudAnchor {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// anchor からのピクセルオフセット (viewport ピクセル単位)。
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct HudOffset {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

/// HUD 要素の外形寸法 (viewport ピクセル単位)。frame.thickness 分は内側に食い込む。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HudSize {
    pub w: f32,
    pub h: f32,
}

/// 外枠の太さと色。thickness が 0 のときは枠を描画しない。
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct HudFrame {
    #[serde(default)]
    pub thickness: f32,
    #[serde(default)]
    pub color: HexColor,
}

/// ゲージが減っていく向き。LTR/RTL は size の幅、TTB/BTT は高さをスケールする。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillDirection {
    #[default]
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
}

/// 1 本の HP バーをいくつのゲージに分けて見せるかの規則。
/// - `FixedCount(n)`: 常に n 等分する (max HP に依らず本数固定)
/// - `PerUnit(n)`: 1 ゲージ = n HP として max HP / n 本に分ける (端数は最終本数で吸収)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GaugeStep {
    FixedCount(u32),
    PerUnit(u32),
}

impl Default for GaugeStep {
    fn default() -> Self {
        Self::FixedCount(1)
    }
}

/// "#RRGGBB" / "#RRGGBBAA" 文字列として YAML に格納する RGBA 色。
///
/// Default は完全不透明黒 (`#000000`)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HexColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for HexColor {
    fn default() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }
}

impl HexColor {
    /// "#RRGGBB" または "#RRGGBBAA" 文字列を parse する。
    ///
    /// # Errors
    /// `#` 始まりでない / 長さが 6 or 8 でない / 16進数として無効、のいずれかで失敗。
    pub fn parse(s: &str) -> Result<Self, String> {
        let trimmed = s.trim();
        let body = trimmed
            .strip_prefix('#')
            .ok_or_else(|| format!("color must start with '#': {trimmed}"))?;
        let hex = |a: usize, b: usize| -> Result<u8, String> {
            u8::from_str_radix(&body[a..b], 16).map_err(|e| format!("invalid hex digit: {e}"))
        };
        match body.len() {
            6 => Ok(Self {
                r: hex(0, 2)?,
                g: hex(2, 4)?,
                b: hex(4, 6)?,
                a: 255,
            }),
            8 => Ok(Self {
                r: hex(0, 2)?,
                g: hex(2, 4)?,
                b: hex(4, 6)?,
                a: hex(6, 8)?,
            }),
            _ => Err(format!("color must be #RRGGBB or #RRGGBBAA: {trimmed}")),
        }
    }

    /// "#RRGGBB" (alpha=255) または "#RRGGBBAA" の文字列に整形する。
    #[must_use]
    pub fn to_hex_string(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

impl Serialize for HexColor {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_hex_string())
    }
}

impl<'de> Deserialize<'de> for HexColor {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl From<HexColor> for Color {
    fn from(c: HexColor) -> Self {
        Color::srgba(
            f32::from(c.r) / 255.0,
            f32::from(c.g) / 255.0,
            f32::from(c.b) / 255.0,
            f32::from(c.a) / 255.0,
        )
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // YAML round-trip では ビット一致を期待する
#[allow(clippy::panic)] // refutable let-else の fallback として明示的に panic させる
mod tests {
    use super::*;

    #[test]
    fn resolution_default_matches_engine_default() {
        let r = Resolution::default();
        assert_eq!(r.width, 384);
        assert_eq!(r.height, 216);
    }

    #[test]
    fn project_default_is_empty_with_default_resolution() {
        let p = Project::default();
        assert!(p.name.is_empty());
        assert_eq!(p.resolution, Resolution::default());
        assert!(p.players.is_empty());
        assert!(p.opponents.is_empty());
        assert!(p.allies.is_empty());
        assert!(p.levels.is_empty());
        assert!(p.hud.elements.is_empty());
    }

    #[test]
    fn hud_element_round_trips_through_yaml() {
        let yaml = r##"elements:
  - kind: player_hp_bar
    anchor: top_left
    offset: { x: 16.0, "y": 16.0 }
    size:   { w: 200.0, h: 16.0 }
    frame:  { thickness: 1.0, color: "#000000" }
    bg_color: "#000000a0"
    fg_color: "#e62626"
    fill_direction: left_to_right
    gauge_step: { fixed_count: 1 }
    gauge_gap: 0.0
"##;
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        assert_eq!(hud.elements.len(), 1);
        let HudElement::PlayerHpBar(cfg) = &hud.elements[0] else {
            panic!("expected player_hp_bar");
        };
        assert_eq!(cfg.anchor, HudAnchor::TopLeft);
        assert_eq!(cfg.offset, HudOffset { x: 16.0, y: 16.0 });
        assert_eq!(cfg.size, HudSize { w: 200.0, h: 16.0 });
        assert_eq!(cfg.frame.thickness, 1.0);
        assert_eq!(cfg.frame.color, HexColor::default()); // #000000
        assert_eq!(cfg.fill_direction, FillDirection::LeftToRight);
        assert_eq!(cfg.gauge_step, GaugeStep::FixedCount(1));
    }

    #[test]
    fn hud_element_minimum_yaml_uses_defaults() {
        // 必須は kind と size だけ、それ以外は default で補える。
        let yaml = r"elements:
  - kind: player_hp_bar
    size: { w: 100.0, h: 10.0 }
";
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerHpBar(cfg) = &hud.elements[0] else {
            panic!("expected player_hp_bar");
        };
        assert_eq!(cfg.target, PlayerId::P1);
        assert_eq!(cfg.anchor, HudAnchor::TopLeft);
        assert_eq!(cfg.offset, HudOffset::default());
        assert_eq!(cfg.frame.thickness, 0.0);
        assert_eq!(cfg.fill_direction, FillDirection::LeftToRight);
        assert_eq!(cfg.gauge_step, GaugeStep::FixedCount(1));
        assert_eq!(cfg.gauge_gap, 0.0);
    }

    #[test]
    fn hud_element_target_p2_round_trips() {
        // ADR-0030: target を明示した要素が parse 通り。kind ごとに match して読む。
        let yaml = r"elements:
  - kind: player_hp_bar
    target: p2
    anchor: top_right
    size: { w: 100.0, h: 8.0 }
  - kind: player_hp_ring
    target: p3
    size: { w: 32.0, h: 32.0 }
";
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerHpBar(cfg0) = &hud.elements[0] else {
            panic!("expected player_hp_bar");
        };
        let HudElement::PlayerHpRing(cfg1) = &hud.elements[1] else {
            panic!("expected player_hp_ring");
        };
        assert_eq!(cfg0.target, PlayerId::P2);
        assert_eq!(cfg1.target, PlayerId::P3);
    }

    #[test]
    fn enemy_hp_bar_round_trips_with_engagement_link_target() {
        // ADR-0031: enemy_hp_bar + LastEngagedBy(p1) + anchor_to で engagement-link を表現する。
        let yaml = r#"elements:
  - kind: player_hp_bar
    id: p1_hp
    target: p1
    anchor: top_left
    offset: { x: 16.0, "y": 16.0 }
    size: { w: 120.0, h: 8.0 }
  - kind: enemy_hp_bar
    target:
      last_engaged_by: p1
    anchor_to:
      id: p1_hp
      edge: bottom_left
    offset: { x: 0.0, "y": 4.0 }
    size: { w: 120.0, h: 6.0 }
"#;
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        assert_eq!(hud.elements[0].id(), Some("p1_hp"));
        let HudElement::EnemyHpBar(cfg) = &hud.elements[1] else {
            panic!("expected enemy_hp_bar");
        };
        assert_eq!(cfg.target, EnemyTarget::LastEngagedBy(PlayerId::P1));
        let at = cfg.anchor_to.as_ref().expect("anchor_to set");
        assert_eq!(at.id, "p1_hp");
        assert_eq!(at.edge, HudAnchor::BottomLeft);
    }

    #[test]
    fn enemy_overhead_hp_bar_round_trips_with_default_tag_filter_and_anchor() {
        // ADR-0032: tag_filter 省略時は None (全 enemy)、vertical_anchor 省略時は image_top。
        let yaml = r"elements:
  - kind: enemy_overhead_hp_bar
    size: { w: 28.0, h: 3.0 }
  - kind: enemy_overhead_hp_bar
    tag_filter: boss
    vertical_anchor: image_bottom
    offset_y: 8.0
    size: { w: 64.0, h: 5.0 }
";
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::EnemyOverheadHpBar(any) = &hud.elements[0] else {
            panic!("expected enemy_overhead_hp_bar");
        };
        assert!(any.tag_filter.is_none());
        assert_eq!(any.vertical_anchor, OverheadVerticalAnchor::ImageTop);
        assert_eq!(any.offset_y, 4.0); // default
        let HudElement::EnemyOverheadHpBar(boss) = &hud.elements[1] else {
            panic!("expected enemy_overhead_hp_bar");
        };
        assert_eq!(boss.tag_filter.as_deref(), Some("boss"));
        assert_eq!(boss.vertical_anchor, OverheadVerticalAnchor::ImageBottom);
        assert_eq!(boss.offset_y, 8.0);
    }

    #[test]
    fn overhead_kind_is_world_anchored_not_screen_anchored() {
        // ADR-0032: screen-anchor 系の variant は is_screen_anchored() == true、
        // overhead は false。spawn_hud がこの flag で振り分ける。
        let overhead = HudElement::EnemyOverheadHpBar(EnemyOverheadHpBarConfig::default());
        let player_bar = HudElement::PlayerHpBar(PlayerHpBarConfig::default());
        assert!(!overhead.is_screen_anchored());
        assert!(player_bar.is_screen_anchored());
    }

    #[test]
    fn enemy_hp_bar_target_tag_and_nth_round_trip() {
        let yaml = r"elements:
  - kind: enemy_hp_bar
    target:
      tag: boss
    anchor: bottom
    size: { w: 240.0, h: 14.0 }
  - kind: enemy_hp_bar
    target:
      nth_enemy: 2
    anchor: top
    size: { w: 80.0, h: 6.0 }
";
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::EnemyHpBar(boss) = &hud.elements[0] else {
            panic!("expected enemy_hp_bar");
        };
        assert_eq!(boss.target, EnemyTarget::Tag("boss".to_string()));
        let HudElement::EnemyHpBar(nth) = &hud.elements[1] else {
            panic!("expected enemy_hp_bar");
        };
        assert_eq!(nth.target, EnemyTarget::NthEnemy(2));
    }

    #[test]
    fn gauge_step_per_unit_round_trips() {
        let yaml = r"elements:
  - kind: player_hp_bar
    size: { w: 100.0, h: 10.0 }
    gauge_step: { per_unit: 100 }
";
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerHpBar(cfg) = &hud.elements[0] else {
            panic!("expected player_hp_bar");
        };
        assert_eq!(cfg.gauge_step, GaugeStep::PerUnit(100));
    }

    #[test]
    fn player_hp_ring_round_trips_through_yaml() {
        let yaml = r##"elements:
  - kind: player_hp_ring
    anchor: top_left
    offset: { x: 16.0, "y": 16.0 }
    size:   { w: 48.0, h: 48.0 }
    frame:  { thickness: 1.0, color: "#000000" }
    bg_color: "#00000099"
    fg_color: "#e62626"
    start_angle: 15.0
    sweep_extent: 330.0
    ring_thickness: 8.0
    direction: clockwise
    gauge_step: { fixed_count: 1 }
    gauge_gap: 0.0
"##;
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerHpRing(cfg) = &hud.elements[0] else {
            panic!("expected player_hp_ring");
        };
        assert_eq!(cfg.size, HudSize { w: 48.0, h: 48.0 });
        assert_eq!(cfg.start_angle, 15.0);
        assert_eq!(cfg.sweep_extent, 330.0);
        assert_eq!(cfg.ring_thickness, 8.0);
        assert_eq!(cfg.direction, RingDirection::Clockwise);
    }

    #[test]
    fn player_icon_round_trips_through_yaml_with_state_sprites() {
        // ADR-0033: state_sprites は Role 名 (snake_case) を key にして HashMap として読む。
        // 主要 state 4 つ + 振動 (on_damage / on_attack_hit) を持つ最小例。
        let yaml = r##"elements:
  - kind: player_icon
    id: p1_icon
    target: p1
    anchor: top_left
    offset: { x: 8.0, "y": 8.0 }
    size: { w: 40.0, h: 40.0 }
    frame: { thickness: 1.0, color: "#000000" }
    bg_color: "#00000080"
    sprite_group_number: 100
    default_sprite_index: 0
    state_sprites:
      idle: 0
      walk: 1
      attack: 2
      hit: 3
    shake:
      on_damage:
        duration_ms: 200
        shake_x: 0
        shake_y: 3
        count: 4
        decay: 1.0
      on_attack_hit:
        duration_ms: 80
        shake_x: 2
        shake_y: 0
        count: 2
        decay: 0.5
"##;
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerIcon(cfg) = &hud.elements[0] else {
            panic!("expected player_icon");
        };
        assert_eq!(cfg.id.as_deref(), Some("p1_icon"));
        assert_eq!(cfg.target, PlayerId::P1);
        assert_eq!(cfg.sprite_group_number, 100);
        assert_eq!(cfg.default_sprite_index, 0);
        assert_eq!(cfg.state_sprites.get(&Role::Idle), Some(&0));
        assert_eq!(cfg.state_sprites.get(&Role::Walk), Some(&1));
        assert_eq!(cfg.state_sprites.get(&Role::Attack), Some(&2));
        assert_eq!(cfg.state_sprites.get(&Role::Hit), Some(&3));
        let on_damage = cfg.shake.on_damage.expect("on_damage present");
        assert_eq!(on_damage.duration_ms, 200);
        assert_eq!(on_damage.shake_y, 3);
        assert_eq!(on_damage.count, 4);
        let on_hit = cfg.shake.on_attack_hit.expect("on_attack_hit present");
        assert_eq!(on_hit.shake_x, 2);
        assert_eq!(on_hit.count, 2);
    }

    #[test]
    fn player_icon_minimum_yaml_uses_defaults() {
        // 必須は kind と size と sprite_group_number だけ。
        // state_sprites 空でも parse でき、shake は None / None になる。
        let yaml = r"elements:
  - kind: player_icon
    size: { w: 32.0, h: 32.0 }
    sprite_group_number: 5
";
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerIcon(cfg) = &hud.elements[0] else {
            panic!("expected player_icon");
        };
        assert_eq!(cfg.target, PlayerId::P1);
        assert_eq!(cfg.anchor, HudAnchor::TopLeft);
        assert_eq!(cfg.sprite_group_number, 5);
        assert_eq!(cfg.default_sprite_index, 0);
        assert!(cfg.state_sprites.is_empty());
        assert!(cfg.shake.on_damage.is_none());
        assert!(cfg.shake.on_attack_hit.is_none());
    }

    #[test]
    fn player_icon_is_screen_anchored_and_exposes_helpers() {
        // PlayerIcon は screen-anchored 系。anchor / size / id / offset の helper が
        // config をそのまま返すこと。
        let mut cfg = PlayerIconConfig {
            sprite_group_number: 42,
            ..PlayerIconConfig::default()
        };
        cfg.id = Some("icon_root".to_string());
        cfg.size = HudSize { w: 24.0, h: 24.0 };
        cfg.anchor = HudAnchor::BottomRight;
        cfg.offset = HudOffset { x: -8.0, y: -8.0 };
        let el = HudElement::PlayerIcon(cfg);
        assert!(el.is_screen_anchored());
        assert_eq!(el.anchor(), HudAnchor::BottomRight);
        assert_eq!(el.size(), HudSize { w: 24.0, h: 24.0 });
        assert_eq!(el.id(), Some("icon_root"));
        assert_eq!(el.offset(), HudOffset { x: -8.0, y: -8.0 });
    }

    #[test]
    fn player_hp_ring_minimum_yaml_uses_defaults() {
        let yaml = r"elements:
  - kind: player_hp_ring
    size: { w: 48.0, h: 48.0 }
";
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerHpRing(cfg) = &hud.elements[0] else {
            panic!("expected player_hp_ring");
        };
        assert_eq!(cfg.start_angle, 0.0);
        assert_eq!(cfg.sweep_extent, 360.0);
        assert_eq!(cfg.ring_thickness, 6.0);
        assert_eq!(cfg.direction, RingDirection::Clockwise);
    }

    #[test]
    fn hex_color_parses_rgb_and_rgba() {
        assert_eq!(
            HexColor::parse("#ff8800").expect("parses"),
            HexColor {
                r: 0xff,
                g: 0x88,
                b: 0x00,
                a: 255
            }
        );
        assert_eq!(
            HexColor::parse("#ff880080").expect("parses"),
            HexColor {
                r: 0xff,
                g: 0x88,
                b: 0x00,
                a: 0x80
            }
        );
    }

    #[test]
    fn hex_color_rejects_bad_input() {
        assert!(HexColor::parse("ff8800").is_err());
        assert!(HexColor::parse("#fff").is_err());
        assert!(HexColor::parse("#zzzzzz").is_err());
    }

    #[test]
    fn hex_color_to_string_omits_alpha_when_opaque() {
        let c = HexColor {
            r: 0xff,
            g: 0x88,
            b: 0x00,
            a: 255,
        };
        assert_eq!(c.to_hex_string(), "#ff8800");
        let c = HexColor {
            r: 0xff,
            g: 0x88,
            b: 0x00,
            a: 0x80,
        };
        assert_eq!(c.to_hex_string(), "#ff880080");
    }
}
