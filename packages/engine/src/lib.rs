//! Engine ライブラリ root。bin (`src/main.rs`) はこの [`entrypoint`] を呼び出すだけ。
//!
//! FSD レイヤー: `app` / `entities` / `features` / `scenes` / `shared`。
//! editor-desktop と同じ構成を engine 側でも踏襲する。
mod app;
pub use app::{SceneState, entrypoint, register_engine_plugins};

pub(crate) mod entities;
pub(crate) mod features;
pub(crate) mod scenes;
pub(crate) mod shared;

// ADR-0041 — smoke test が `init_resource::<AudioSettings>()` で本番 SettingsPlugin
// 経由の挿入を代替できるよう、ユーザー設定の Resource 型のみ再公開する。
pub use shared::settings::{AudioSettings, WindowSettings};
