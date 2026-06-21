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
