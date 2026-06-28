use super::app_root::AppRoot;
use dioxus::desktop::{Config, WindowBuilder};
use tracing::Level;

/// # Panics
///
/// - ロガーの初期化に失敗した場合。
pub fn entrypoint() {
    dioxus::logger::init(Level::DEBUG).expect("Loggerの初期化に失敗。");

    let window_config = Config::new()
        .with_window(
            WindowBuilder::new()
                .with_title("Bebeu Editor")
                .with_maximized(true)
                .with_always_on_top(false),
        )
        .with_menu(None)
        .with_disable_context_menu(true);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(window_config)
        .launch(AppRoot);
}
