use dioxus::prelude::*;

use crate::shared::{translate, translate_args};

use super::Preferences;

/// Signal<Preferences> を Dioxus context で配布する。`AppMain` で 1 度だけ呼ぶ。
pub fn use_preferences_provider(initial: Preferences) -> Signal<Preferences> {
    use_context_provider(|| Signal::new(initial))
}

/// 配布済みの Signal<Preferences> を取得する。pages / widgets / features から呼ぶ。
pub fn use_preferences() -> Signal<Preferences> {
    use_context::<Signal<Preferences>>()
}

/// locale signal を購読する reactive な翻訳関数を返す。
/// 戻り値は Copy なので、handler や rsx の `{t(...)}` で再利用できる。
/// 内部の `translate` 呼び出しは現在の `rust_i18n` thread-local locale を読むので、
/// `app_root` の `use_effect` が事前に `apply_locale` を済ませている必要がある。
#[must_use]
pub fn use_t() -> impl Fn(&str) -> String + Copy + 'static {
    let prefs = use_preferences();
    move |key: &str| {
        let _ = prefs.read().locale;
        translate(key)
    }
}

/// `use_t` の placeholder 付き版。`%{name}` を args で差し替える。
#[must_use]
pub fn use_t_args() -> impl Fn(&str, &[(&str, &str)]) -> String + Copy + 'static {
    let prefs = use_preferences();
    move |key: &str, args: &[(&str, &str)]| {
        let _ = prefs.read().locale;
        translate_args(key, args)
    }
}
