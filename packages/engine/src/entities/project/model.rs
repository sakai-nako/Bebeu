//! Project の型定義 (FSD: model segment)。
//!
//! `runtime/data/projects/{name}.yml` の構造に対応する。
//! ロード処理は隣の [`super::api`] に分離している。
use bevy::prelude::{Color, Resource};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HudElement {
    PlayerHpBar(PlayerHpBarConfig),
}

impl HudElement {
    /// 描画位置の anchor を返す。variant が増えてもここで一括取得できるようにする。
    #[must_use]
    pub fn anchor(&self) -> HudAnchor {
        match self {
            HudElement::PlayerHpBar(c) => c.anchor,
        }
    }

    /// 描画位置の offset を返す。variant が増えてもここで一括取得できるようにする。
    #[must_use]
    pub fn offset(&self) -> HudOffset {
        match self {
            HudElement::PlayerHpBar(c) => c.offset,
        }
    }
}

/// Player HP バーの表示設定。size は外形 bbox で、`frame.thickness` 分だけ
/// ゲージ描画領域が内側に縮む (= frame は size の内側に食い込む)。
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
        let HudElement::PlayerHpBar(cfg) = hud.elements[0];
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
        let HudElement::PlayerHpBar(cfg) = hud.elements[0];
        assert_eq!(cfg.anchor, HudAnchor::TopLeft);
        assert_eq!(cfg.offset, HudOffset::default());
        assert_eq!(cfg.frame.thickness, 0.0);
        assert_eq!(cfg.fill_direction, FillDirection::LeftToRight);
        assert_eq!(cfg.gauge_step, GaugeStep::FixedCount(1));
        assert_eq!(cfg.gauge_gap, 0.0);
    }

    #[test]
    fn gauge_step_per_unit_round_trips() {
        let yaml = r"elements:
  - kind: player_hp_bar
    size: { w: 100.0, h: 10.0 }
    gauge_step: { per_unit: 100 }
";
        let hud: Hud = serde_saphyr::from_str(yaml).expect("hud yaml parses");
        let HudElement::PlayerHpBar(cfg) = hud.elements[0];
        assert_eq!(cfg.gauge_step, GaugeStep::PerUnit(100));
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
