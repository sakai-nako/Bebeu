use dioxus::prelude::*;

use super::Preferences;

/// Signal<Preferences> を Dioxus context で配布する。`AppMain` で 1 度だけ呼ぶ。
pub fn use_preferences_provider(initial: Preferences) -> Signal<Preferences> {
    use_context_provider(|| Signal::new(initial))
}

/// 配布済みの Signal<Preferences> を取得する。pages / widgets / features から呼ぶ。
pub fn use_preferences() -> Signal<Preferences> {
    use_context::<Signal<Preferences>>()
}
