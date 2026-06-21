//! Engine の app レイヤ (FSD: Application slice)。
//!
//! Bevy `App` のブートストラップ、Scene 遷移、CLI 引数解釈を担う。
//! editor-desktop の `app.rs` と同様、サブモジュール宣言と公開 API の re-export のみ。
mod entrypoint;
pub use entrypoint::{
    EngineConfig, RunOptions, SceneState, entrypoint, register_engine_plugins, run,
};
