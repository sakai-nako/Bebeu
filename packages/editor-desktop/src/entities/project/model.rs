use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 論理解像度のデフォルト幅 (px)。
const DEFAULT_RESOLUTION_WIDTH: u32 = 640;

/// 論理解像度のデフォルト高 (px)。
const DEFAULT_RESOLUTION_HEIGHT: u32 = 360;

/// 1 つのプロジェクト設定。`workspace/data/projects/{name}.yml` に永続化される。
///
/// 1 workspace に複数 Project を並べ、engine 起動時に `--project <name>` で指定する。
/// Character / Level の master pool は workspace/data/characters/ と workspace/data/levels/
/// に共有で置かれ、Editor 上では Project を介さず直接編集できる。Project は engine 起動の
/// preset (どの player / opponent / level で起動するか / HUD レイアウトをどう組むか) として
/// 機能し、Editor 内の Character / Level 一覧をフィルタすることはしない。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// ディレクトリ名 (= ファイル名 stem) から復元される。YAML には書かない。
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub resolution: Resolution,
    #[serde(default)]
    pub players: Vec<String>,
    #[serde(default)]
    pub opponents: Vec<String>,
    #[serde(default)]
    pub levels: Vec<String>,
    #[serde(default)]
    pub hud: Hud,
}

/// gameplay 中の HUD レイアウト (ADR-0029)。
/// 要素は配列で、project ごとに自由に並べる。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hud {
    #[serde(default)]
    pub elements: Vec<HudElement>,
}

/// HUD 1 要素。internally-tagged enum で `kind:` が YAML 上の判別キーになる (ADR-0029)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HudElement {
    PlayerHpBar(PlayerHpBarConfig),
}

impl HudElement {
    /// UI 上の表示名 (dropdown 等)。
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            HudElement::PlayerHpBar(_) => "Player HP bar",
        }
    }

    /// kind 識別子 (snake_case)。dropdown の value に使う。
    #[must_use]
    pub fn kind_value(&self) -> &'static str {
        match self {
            HudElement::PlayerHpBar(_) => "player_hp_bar",
        }
    }

    /// 全 kind の一覧 (default 値付き)。「+ 要素を追加」の選択肢を生成する用途。
    #[must_use]
    pub fn all_kinds() -> &'static [HudKindOption] {
        &[HudKindOption {
            value: "player_hp_bar",
            label: "Player HP bar",
        }]
    }

    /// kind 識別子から default 値の HudElement を作る。
    #[must_use]
    pub fn default_for_kind(value: &str) -> Option<Self> {
        match value {
            "player_hp_bar" => Some(Self::PlayerHpBar(PlayerHpBarConfig::default())),
            _ => None,
        }
    }

    /// 描画位置の anchor を返す。
    #[must_use]
    pub fn anchor(&self) -> HudAnchor {
        match self {
            HudElement::PlayerHpBar(c) => c.anchor,
        }
    }

    /// 描画位置の offset を返す。
    #[must_use]
    pub fn offset(&self) -> HudOffset {
        match self {
            HudElement::PlayerHpBar(c) => c.offset,
        }
    }
}

/// UI の dropdown 用 (value, label) ペア。
#[derive(Debug, Clone, Copy)]
pub struct HudKindOption {
    pub value: &'static str,
    pub label: &'static str,
}

/// Player HP バーの表示設定。size は外形 bbox で、frame.thickness 分だけゲージ
/// 描画領域が内側に縮む (frame は size の内側に食い込む)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlayerHpBarConfig {
    #[serde(default)]
    pub anchor: HudAnchor,
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
            anchor: HudAnchor::TopLeft,
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

impl HudAnchor {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            HudAnchor::TopLeft => "Top-left",
            HudAnchor::Top => "Top",
            HudAnchor::TopRight => "Top-right",
            HudAnchor::Left => "Left",
            HudAnchor::Center => "Center",
            HudAnchor::Right => "Right",
            HudAnchor::BottomLeft => "Bottom-left",
            HudAnchor::Bottom => "Bottom",
            HudAnchor::BottomRight => "Bottom-right",
        }
    }

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            HudAnchor::TopLeft => "top_left",
            HudAnchor::Top => "top",
            HudAnchor::TopRight => "top_right",
            HudAnchor::Left => "left",
            HudAnchor::Center => "center",
            HudAnchor::Right => "right",
            HudAnchor::BottomLeft => "bottom_left",
            HudAnchor::Bottom => "bottom",
            HudAnchor::BottomRight => "bottom_right",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<HudAnchor> {
        match s {
            "top_left" => Some(HudAnchor::TopLeft),
            "top" => Some(HudAnchor::Top),
            "top_right" => Some(HudAnchor::TopRight),
            "left" => Some(HudAnchor::Left),
            "center" => Some(HudAnchor::Center),
            "right" => Some(HudAnchor::Right),
            "bottom_left" => Some(HudAnchor::BottomLeft),
            "bottom" => Some(HudAnchor::Bottom),
            "bottom_right" => Some(HudAnchor::BottomRight),
            _ => None,
        }
    }

    pub const ALL: &'static [HudAnchor] = &[
        HudAnchor::TopLeft,
        HudAnchor::Top,
        HudAnchor::TopRight,
        HudAnchor::Left,
        HudAnchor::Center,
        HudAnchor::Right,
        HudAnchor::BottomLeft,
        HudAnchor::Bottom,
        HudAnchor::BottomRight,
    ];
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

impl FillDirection {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            FillDirection::LeftToRight => "Left → Right",
            FillDirection::RightToLeft => "Right → Left",
            FillDirection::TopToBottom => "Top → Bottom",
            FillDirection::BottomToTop => "Bottom → Top",
        }
    }

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            FillDirection::LeftToRight => "left_to_right",
            FillDirection::RightToLeft => "right_to_left",
            FillDirection::TopToBottom => "top_to_bottom",
            FillDirection::BottomToTop => "bottom_to_top",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<FillDirection> {
        match s {
            "left_to_right" => Some(FillDirection::LeftToRight),
            "right_to_left" => Some(FillDirection::RightToLeft),
            "top_to_bottom" => Some(FillDirection::TopToBottom),
            "bottom_to_top" => Some(FillDirection::BottomToTop),
            _ => None,
        }
    }

    pub const ALL: &'static [FillDirection] = &[
        FillDirection::LeftToRight,
        FillDirection::RightToLeft,
        FillDirection::TopToBottom,
        FillDirection::BottomToTop,
    ];
}

/// 1 本の HP バーをいくつのゲージに分けて見せるかの規則。
/// - `FixedCount(n)`: 常に n 等分する (max HP に依らず本数固定)
/// - `PerUnit(n)`: 1 ゲージ = n HP として max HP / n 本に分ける
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

impl GaugeStep {
    /// UI の tag dropdown 用。
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            GaugeStep::FixedCount(_) => "fixed_count",
            GaugeStep::PerUnit(_) => "per_unit",
        }
    }

    /// UI 上で表示する単一の整数値 (本数 or HP/本)。
    #[must_use]
    pub fn amount(self) -> u32 {
        match self {
            GaugeStep::FixedCount(n) | GaugeStep::PerUnit(n) => n,
        }
    }

    /// 同じ amount を保持したまま variant だけ切り替える。
    #[must_use]
    pub fn with_value(value: &str, amount: u32) -> Option<Self> {
        match value {
            "fixed_count" => Some(Self::FixedCount(amount)),
            "per_unit" => Some(Self::PerUnit(amount)),
            _ => None,
        }
    }

    /// amount だけ差し替える。
    #[must_use]
    pub fn with_amount(self, amount: u32) -> Self {
        match self {
            GaugeStep::FixedCount(_) => Self::FixedCount(amount),
            GaugeStep::PerUnit(_) => Self::PerUnit(amount),
        }
    }
}

/// "#RRGGBB" / "#RRGGBBAA" 文字列として YAML に格納する RGBA 色。
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

    /// `<input type="color">` 用の "#RRGGBB" を返す (alpha は無視)。
    #[must_use]
    pub fn to_rgb_hex_string(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// alpha チャンネルを 0.0..=1.0 で返す。
    #[must_use]
    pub fn alpha_f32(self) -> f32 {
        f32::from(self.a) / 255.0
    }

    /// alpha を 0.0..=1.0 で差し替える。
    #[must_use]
    pub fn with_alpha_f32(self, a: f32) -> Self {
        // clamp(0,1) * 255 → round 後は 0..=255 に収まる
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let a = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self { a, ..self }
    }

    /// RGB だけ別 hex 文字列から取り直す (alpha は保持)。
    pub fn with_rgb_hex(self, hex: &str) -> Result<Self, String> {
        let rgb = Self::parse(hex)?;
        Ok(Self {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
            a: self.a,
        })
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

/// 論理解像度（描画バッファのサイズ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            width: DEFAULT_RESOLUTION_WIDTH,
            height: DEFAULT_RESOLUTION_HEIGHT,
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // YAML round-trip では ビット一致を期待する
mod tests {
    use super::*;

    #[test]
    fn hud_element_round_trips_through_yaml() {
        let yaml = r##"elements:
  - kind: player_hp_bar
    anchor: top_left
    offset: { x: 16.0, "y": 16.0 }
    size:   { w: 120.0, h: 8.0 }
    frame:  { thickness: 1.0, color: "#000000" }
    bg_color: "#000000a0"
    fg_color: "#e62626"
    fill_direction: left_to_right
    gauge_step: { fixed_count: 1 }
    gauge_gap: 0.0
"##;
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerHpBar(cfg) = hud.elements[0];
        assert_eq!(cfg.size, HudSize { w: 120.0, h: 8.0 });
        assert_eq!(cfg.frame.thickness, 1.0);
        assert_eq!(cfg.fill_direction, FillDirection::LeftToRight);
        assert_eq!(cfg.gauge_step, GaugeStep::FixedCount(1));
    }

    #[test]
    fn hex_color_round_trips() {
        let c = HexColor {
            r: 0xff,
            g: 0x88,
            b: 0x00,
            a: 0x80,
        };
        let s = c.to_hex_string();
        assert_eq!(s, "#ff880080");
        assert_eq!(HexColor::parse(&s).expect("parses"), c);
    }
}
