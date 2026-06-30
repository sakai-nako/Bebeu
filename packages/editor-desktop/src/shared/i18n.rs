use serde::{Deserialize, Serialize};

// `rust_i18n::i18n!()` macro は crate root で展開される必要があるため lib.rs に置いている。

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    Ja,
    En,
}

impl Locale {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ja => "ja",
            Self::En => "en",
        }
    }

    #[must_use]
    pub fn all() -> &'static [Locale] {
        &[Locale::Ja, Locale::En]
    }
}

/// OS locale (`en-US` / `ja-JP` 等) から先頭 2 文字を見て `Locale` を判定する。
/// 取得できない / 未対応言語の場合は `Locale::default()` (= Ja)。
///
/// 初回起動 (`preferences.yml` が存在しない) ケースだけ呼ぶ想定。
/// 既存 yml には `Locale::default()` で serde 補完するので、既存ユーザーの ja は上書きされない。
#[must_use]
pub fn detect_default_locale() -> Locale {
    let Some(raw) = sys_locale::get_locale() else {
        return Locale::default();
    };
    match raw.get(..2) {
        Some("ja") => Locale::Ja,
        Some("en") => Locale::En,
        _ => Locale::default(),
    }
}

/// rust_i18n の thread-local locale を更新する。Dioxus 側の `use_effect` から呼ぶ。
pub fn apply_locale(locale: Locale) {
    rust_i18n::set_locale(locale.as_str());
}

/// `rust_i18n::t!` の薄いラッパー。component 側で `let t = use_t(); rsx! { "{t(\"key\")}" }`
/// のように呼ぶ。`use_t` の hook は `entities/preference/provider.rs` 側に置く
/// (entities → shared の依存方向を維持するため)。
#[must_use]
pub fn translate(key: &str) -> String {
    rust_i18n::t!(key).to_string()
}

/// placeholder 付き翻訳。catalog 内の `%{name}` を引数で差し替える。
/// 例: `translate_args("app.startup_error_message", &[("error", err.as_str())])`
#[must_use]
pub fn translate_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut out = rust_i18n::t!(key).to_string();
    for (name, value) in args {
        out = out.replace(&format!("%{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_returns_ja_by_default() {
        apply_locale(Locale::Ja);
        assert_eq!(translate("preferences.close"), "閉じる");
    }

    #[test]
    fn translate_returns_en_after_switch() {
        apply_locale(Locale::En);
        assert_eq!(translate("preferences.close"), "Close");
        apply_locale(Locale::Ja);
    }

    #[test]
    fn translate_args_substitutes_placeholder() {
        apply_locale(Locale::En);
        let out = translate_args("app.startup_error_message", &[("error", "boom")]);
        assert_eq!(out, "Failed to load configuration: boom");
        apply_locale(Locale::Ja);
    }
}
