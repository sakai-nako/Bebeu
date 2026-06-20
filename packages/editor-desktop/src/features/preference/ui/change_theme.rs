use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::preference::{Preferences, PreferencesRepository, Theme, use_preferences};

const THEMES: &[(Theme, &str)] = &[(Theme::Emerald, "Emerald (light)"), (Theme::Dark, "Dark")];

#[component]
pub fn ChangeThemeSelect() -> Element {
    let repo = use_context::<Arc<dyn PreferencesRepository>>();
    let mut preferences = use_preferences();
    let mut error = use_signal(|| None::<String>);

    let current = preferences.read().theme;

    let on_change = move |evt: Event<FormData>| {
        let value = evt.value();
        let new_theme = match value.as_str() {
            "emerald" => Theme::Emerald,
            "dark" => Theme::Dark,
            other => {
                error.set(Some(format!("未知のテーマ: {other}")));
                return;
            }
        };

        let new_prefs = Preferences {
            theme: new_theme,
            ..preferences.peek().clone()
        };
        // disk 保存に成功してから signal を更新（disk と memory の乖離を避ける）
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
            legend { class: "fieldset-legend", "テーマ" }
            select { class: "select select-bordered w-full", onchange: on_change,
                for (theme, label) in THEMES {
                    option {
                        key: "{theme.as_str()}",
                        value: "{theme.as_str()}",
                        selected: *theme == current,
                        "{label}"
                    }
                }
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        }
    }
}
