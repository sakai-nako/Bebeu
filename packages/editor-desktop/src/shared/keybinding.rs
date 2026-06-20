use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// 修飾キーの集合。`dioxus::events::Modifiers` (bitflags) を user 用構造体に変換した形。
///
/// `meta` は将来の macOS Cmd / Windows Win 用フィールド。MVP は ctrl / shift / alt のみ使う想定。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // 修飾キー (Ctrl/Shift/Alt/Meta) は固定の 4 値で対応
pub struct KeyModifiers {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub meta: bool,
}

impl KeyModifiers {
    #[must_use]
    pub fn is_empty(self) -> bool {
        !self.ctrl && !self.shift && !self.alt && !self.meta
    }
}

/// 修飾キー + メインキー の組。`Display` は人間表示 (例: "Ctrl+S")、`canonical_string` は
/// YAML 等の永続化用形式 (例: "ctrl+s") を返す。
///
/// 永続化は `#[serde(try_from = "String", into = "String")]` で文字列 1 行にする。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KeyBinding {
    pub modifiers: KeyModifiers,
    /// 正規化されたキー名。1 文字のときは ASCII 小文字、それ以外は標準キー名
    /// ("Enter", "Escape", "ArrowLeft", "F1" 等)。
    pub key: String,
}

impl KeyBinding {
    /// 永続化用の正規形を返す (例: "ctrl+shift+s")。
    #[must_use]
    pub fn canonical_string(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.modifiers.ctrl {
            parts.push("ctrl");
        }
        if self.modifiers.shift {
            parts.push("shift");
        }
        if self.modifiers.alt {
            parts.push("alt");
        }
        if self.modifiers.meta {
            parts.push("meta");
        }
        parts.push(&self.key);
        parts.join("+")
    }

    /// Dioxus の `KeyboardEvent` から `KeyBinding` を構築する。
    /// 修飾キー単独の押下 (例: Ctrl だけ) は `None` を返す。
    #[must_use]
    pub fn from_keyboard_event(evt: &dioxus::prelude::KeyboardEvent) -> Option<Self> {
        use dioxus::prelude::{Key, Modifiers, ModifiersInteraction};
        let raw = evt.modifiers();
        let modifiers = KeyModifiers {
            ctrl: raw.contains(Modifiers::CONTROL),
            shift: raw.contains(Modifiers::SHIFT),
            alt: raw.contains(Modifiers::ALT),
            meta: raw.contains(Modifiers::META),
        };
        let key = match evt.key() {
            Key::Control
            | Key::Shift
            | Key::Alt
            | Key::Meta
            | Key::AltGraph
            | Key::Unidentified => return None,
            // 単一スペース文字 (Key::Character(" ")) は parser 側の正規形 "Space" に揃える。
            // これがないと "Space" 表記の binding と event 由来の " " が一致せずマッチしない。
            Key::Character(s) if s == " " => "Space".into(),
            Key::Character(s) if s.chars().count() == 1 => s.to_ascii_lowercase(),
            Key::Character(s) => s,
            other => other.to_string(),
        };
        if key.is_empty() {
            return None;
        }
        Some(Self { modifiers, key })
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        if self.modifiers.ctrl {
            parts.push("Ctrl".into());
        }
        if self.modifiers.shift {
            parts.push("Shift".into());
        }
        if self.modifiers.alt {
            parts.push("Alt".into());
        }
        if self.modifiers.meta {
            parts.push("Meta".into());
        }
        // 1 文字キーは表示用に大文字化、それ以外は標準キー名をそのまま使う
        let display_key = if self.key.chars().count() == 1 {
            self.key.to_ascii_uppercase()
        } else {
            self.key.clone()
        };
        parts.push(display_key);
        write!(f, "{}", parts.join("+"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyParseError(String);

impl fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for KeyParseError {}

impl FromStr for KeyBinding {
    type Err = KeyParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(KeyParseError("空のキーバインドは無効です".into()));
        }
        let parts: Vec<&str> = trimmed.split('+').map(str::trim).collect();
        if parts.iter().any(|p| p.is_empty()) {
            return Err(KeyParseError(format!("不正なキーバインド: {s:?}")));
        }

        let mut modifiers = KeyModifiers::default();
        let mut main: Option<String> = None;
        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "shift" => modifiers.shift = true,
                "alt" | "option" => modifiers.alt = true,
                "meta" | "cmd" | "command" | "win" | "super" => modifiers.meta = true,
                _ => {
                    if main.is_some() {
                        return Err(KeyParseError(format!(
                            "メインキーが複数指定されています: {s:?}"
                        )));
                    }
                    main = Some(normalize_key_name(part));
                }
            }
        }
        let key = main.ok_or_else(|| KeyParseError(format!("メインキーがありません: {s:?}")))?;
        Ok(Self { modifiers, key })
    }
}

impl TryFrom<String> for KeyBinding {
    type Error = KeyParseError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<KeyBinding> for String {
    fn from(kb: KeyBinding) -> String {
        kb.canonical_string()
    }
}

/// 自由入力されたキー名を正規形に揃える。1 文字なら小文字、特殊キーは標準キャメル名。
fn normalize_key_name(raw: &str) -> String {
    if raw.chars().count() == 1 {
        return raw.to_ascii_lowercase();
    }
    let lower = raw.to_ascii_lowercase();
    match lower.as_str() {
        "enter" | "return" => "Enter".into(),
        "escape" | "esc" => "Escape".into(),
        "tab" => "Tab".into(),
        "space" => "Space".into(),
        "backspace" => "Backspace".into(),
        "delete" | "del" => "Delete".into(),
        "insert" | "ins" => "Insert".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        "pageup" => "PageUp".into(),
        "pagedown" => "PageDown".into(),
        "arrowleft" | "left" => "ArrowLeft".into(),
        "arrowright" | "right" => "ArrowRight".into(),
        "arrowup" | "up" => "ArrowUp".into(),
        "arrowdown" | "down" => "ArrowDown".into(),
        other => {
            // F1〜F24
            if let Some(rest) = other.strip_prefix('f')
                && let Ok(n) = rest.parse::<u32>()
                && (1..=24).contains(&n)
            {
                return format!("F{n}");
            }
            // 不明なキー名は元の文字列をそのまま採用
            raw.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_binding_parses_canonical_form() -> Result<(), KeyParseError> {
        let kb: KeyBinding = "ctrl+s".parse()?;
        assert_eq!(
            kb,
            KeyBinding {
                modifiers: KeyModifiers {
                    ctrl: true,
                    ..Default::default()
                },
                key: "s".into(),
            }
        );
        Ok(())
    }

    #[test]
    fn key_binding_parse_is_case_insensitive_for_modifiers() -> Result<(), KeyParseError> {
        let a: KeyBinding = "CTRL+Shift+s".parse()?;
        let b: KeyBinding = "ctrl+shift+S".parse()?;
        assert_eq!(a, b);
        assert!(a.modifiers.ctrl);
        assert!(a.modifiers.shift);
        assert_eq!(a.key, "s");
        Ok(())
    }

    #[test]
    fn key_binding_parse_rejects_empty() {
        assert!("".parse::<KeyBinding>().is_err());
        assert!("   ".parse::<KeyBinding>().is_err());
        assert!("+s".parse::<KeyBinding>().is_err());
        assert!("ctrl+".parse::<KeyBinding>().is_err());
        assert!("ctrl".parse::<KeyBinding>().is_err()); // メインキーなし
    }

    #[test]
    fn key_binding_parse_normalizes_named_keys() -> Result<(), KeyParseError> {
        let kb: KeyBinding = "alt+enter".parse()?;
        assert_eq!(kb.key, "Enter");
        let kb: KeyBinding = "f5".parse()?;
        assert_eq!(kb.key, "F5");
        let kb: KeyBinding = "del".parse()?;
        assert_eq!(kb.key, "Delete");
        Ok(())
    }

    #[test]
    fn key_binding_display_round_trips_with_parse() -> Result<(), KeyParseError> {
        let original = KeyBinding {
            modifiers: KeyModifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            key: "s".into(),
        };
        let displayed = format!("{original}");
        assert_eq!(displayed, "Ctrl+Shift+S");
        let reparsed: KeyBinding = displayed.parse()?;
        assert_eq!(reparsed, original);
        Ok(())
    }

    #[test]
    fn key_binding_canonical_round_trips_with_parse() -> Result<(), KeyParseError> {
        let original = KeyBinding {
            modifiers: KeyModifiers {
                ctrl: true,
                ..Default::default()
            },
            key: "s".into(),
        };
        let canonical = original.canonical_string();
        assert_eq!(canonical, "ctrl+s");
        let reparsed: KeyBinding = canonical.parse()?;
        assert_eq!(reparsed, original);
        Ok(())
    }

    #[test]
    fn key_binding_parse_rejects_multiple_main_keys() {
        assert!("ctrl+s+t".parse::<KeyBinding>().is_err());
    }
}
