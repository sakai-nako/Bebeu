use serde::{Deserialize, Serialize};

use crate::entities::keybinding::KeyBindings;
use crate::shared::ViewControlBindings;

/// SpriteGroup Editor の Undo 履歴のデフォルト上限ステップ数。
const DEFAULT_SPRITE_GROUP_HISTORY_CAPACITY: u32 = 50;

/// Animation Editor の Undo 履歴のデフォルト上限ステップ数。
const DEFAULT_ANIMATION_HISTORY_CAPACITY: u32 = 50;

/// Level Editor の Undo 履歴のデフォルト上限ステップ数。
const DEFAULT_LEVEL_HISTORY_CAPACITY: u32 = 50;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Emerald,
    Dark,
}

impl Theme {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Emerald => "emerald",
            Self::Dark => "dark",
        }
    }
}

pub use crate::shared::Locale;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub theme: Theme,
    /// editor UI の表示言語。`#[serde(default)]` は `Locale::default()` (= Ja) で補完する。
    /// 既存ユーザー (preferences.yml あり) は日本語のまま、初回起動時 (ファイル不在) は
    /// `FilesystemPreferencesRepository::load` 経由で OS locale 検出が走る (ADR-0042)。
    #[serde(default)]
    pub locale: Locale,
    #[serde(default)]
    pub view_controls: ViewControlBindings,
    #[serde(default)]
    pub key_bindings: KeyBindings,
    /// SpriteGroup Editor で保持する Undo 履歴の最大ステップ数。
    /// `#[derive(Default)]` だと u32 が 0 になり履歴が無効化されるので、`Default` は手動実装する。
    #[serde(default = "default_sprite_group_history_capacity")]
    pub sprite_group_history_capacity: u32,
    /// Animation Editor で保持する Undo 履歴の最大ステップ数。
    #[serde(default = "default_animation_history_capacity")]
    pub animation_history_capacity: u32,
    /// Level Editor で保持する Undo 履歴の最大ステップ数。
    #[serde(default = "default_level_history_capacity")]
    pub level_history_capacity: u32,
}

fn default_sprite_group_history_capacity() -> u32 {
    DEFAULT_SPRITE_GROUP_HISTORY_CAPACITY
}

fn default_animation_history_capacity() -> u32 {
    DEFAULT_ANIMATION_HISTORY_CAPACITY
}

fn default_level_history_capacity() -> u32 {
    DEFAULT_LEVEL_HISTORY_CAPACITY
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            locale: Locale::default(),
            view_controls: ViewControlBindings::default(),
            key_bindings: KeyBindings::default(),
            sprite_group_history_capacity: DEFAULT_SPRITE_GROUP_HISTORY_CAPACITY,
            animation_history_capacity: DEFAULT_ANIMATION_HISTORY_CAPACITY,
            level_history_capacity: DEFAULT_LEVEL_HISTORY_CAPACITY,
        }
    }
}
