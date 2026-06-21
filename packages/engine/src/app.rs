//! Engine の app レイヤ (FSD: Application slice)。
//!
//! Bevy `App` のブートストラップ、Scene 遷移、CLI 引数解釈を担う。
//! editor-desktop の `app.rs` と同様、サブモジュール宣言と公開 API の re-export のみ。
mod entrypoint;
mod pixel_perfect;
pub use entrypoint::{RunOptions, SceneState, entrypoint, register_engine_plugins, run};
pub use pixel_perfect::{
    FINAL_PASS_LAYER, FinalPassCamera, FinalPassSprite, PixelPerfectConfig,
    PixelPerfectRenderPlugin, PixelPerfectTarget,
};
