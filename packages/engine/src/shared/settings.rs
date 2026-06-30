//! ユーザー側設定の永続化 (ADR-0041)。
//!
//! Bevy 0.19 で導入された `bevy::settings` (App Settings) を使い、
//! `%LOCALAPPDATA%\com.hack-pleasantness.bebeu.engine\settings.toml`
//! (OS 規約) にユーザー設定を保存する。
//!
//! 当 repo の永続化レイヤは 3 層:
//! - L1 ゲームデータ (workspace_dir/data/*.yml, YAML, ADR-0011)
//! - L2 engine 起動 config (bebeu-engine.yml, YAML, ADR-0016)
//! - **L3 ユーザー設定** (本 module, TOML, ADR-0041)
//!
//! Settings は `SettingsPlugin` add 時点で同期 load され、各 group の Resource
//! として world に挿入される。保存は `SaveSettingsDeferred` (debounce) /
//! `SaveSettingsSync::IfChanged` (on exit) を Command として queue する。
//!
//! `Reflect` derive は engine crate 内ではこの module でだけ使う (App Settings
//! の要件)。他用途には広げない。
use bevy::math::{IVec2, UVec2};
use bevy::prelude::*;
use bevy::settings::{ReflectSettingsGroup, SettingsGroup};

/// `SettingsPlugin` に渡す reverse-domain app 名 (ADR-0041)。
/// この名前で OS の preferences directory にサブディレクトリが作られる:
/// - Windows: `%LOCALAPPDATA%\com.hack-pleasantness.bebeu.engine\`
/// - macOS:   `~/Library/Preferences/com.hack-pleasantness.bebeu.engine/`
/// - Linux:   `~/.config/com.hack-pleasantness.bebeu.engine/`
pub const APP_NAME: &str = "com.hack-pleasantness.bebeu.engine";

/// Window の位置 / サイズ / fullscreen state を起動間で保つ。
///
/// 初回起動時 (= TOML 不在) は default (= 全 None / false)。entrypoint 側で
/// `position` `size` が None なら DefaultPlugins の `Window` 初期値をそのまま使う。
#[derive(Resource, SettingsGroup, Reflect, Default, Clone, PartialEq)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "window")]
pub struct WindowSettings {
    pub position: Option<IVec2>,
    pub size: Option<UVec2>,
    pub fullscreen: bool,
}

/// Audio の master gain (0.0-1.0)。SE 発火時に `Volume::Linear` に掛ける
/// (`features/character/sound.rs` の dispatch システム)。
///
/// BGM は現状未実装。BGM 系を入れる時に `bgm_volume` / `sfx_volume` の分離を検討。
#[derive(Resource, SettingsGroup, Reflect, Clone, PartialEq)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "audio")]
pub struct AudioSettings {
    pub master_volume: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { master_volume: 1.0 }
    }
}
