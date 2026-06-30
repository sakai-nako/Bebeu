use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::preference::{
    Locale, Preferences, PreferencesRepository, use_preferences, use_t, use_t_args,
};

#[component]
pub fn ChangeLocaleSelect() -> Element {
    let repo = use_context::<Arc<dyn PreferencesRepository>>();
    let mut preferences = use_preferences();
    let mut error = use_signal(|| None::<String>);
    let t = use_t();
    let t_args = use_t_args();

    let current = preferences.read().locale;

    let on_change = move |evt: Event<FormData>| {
        let value = evt.value();
        let new_locale = match value.as_str() {
            "ja" => Locale::Ja,
            "en" => Locale::En,
            other => {
                error.set(Some(t_args(
                    "preferences.locale_unknown",
                    &[("value", other)],
                )));
                return;
            }
        };

        let new_prefs = Preferences {
            locale: new_locale,
            ..preferences.peek().clone()
        };
        // disk 保存に成功してから signal を更新 (change_theme.rs と同じ pattern: ADR-0012)
        match repo.save(&new_prefs) {
            Ok(()) => {
                preferences.set(new_prefs);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    rsx! {
        fieldset { class: "fieldset",
            legend { class: "fieldset-legend", "{t(\"preferences.locale\")}" }
            select { class: "select select-bordered w-full", onchange: on_change,
                for locale in Locale::all() {
                    option {
                        key: "{locale.as_str()}",
                        value: "{locale.as_str()}",
                        selected: *locale == current,
                        "{t(locale_label_key(*locale))}"
                    }
                }
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        }
    }
}

fn locale_label_key(locale: Locale) -> &'static str {
    match locale {
        Locale::Ja => "preferences.locale_ja",
        Locale::En => "preferences.locale_en",
    }
}
