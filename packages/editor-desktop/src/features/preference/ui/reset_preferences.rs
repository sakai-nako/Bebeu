use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::preference::{Preferences, PreferencesRepository, use_preferences};

/// 「キーコンフィグ以外」を一括でデフォルトに戻すボタン。
///
/// `Preferences::default()` をベースにしつつ、現在の `key_bindings` だけは持ち越す。
/// キーコンフィグのリセット系は `EditKeyBindings` 側に集約しており、ここでは扱わない。
#[component]
pub fn ResetPreferencesButton() -> Element {
    let repo = use_context::<Arc<dyn PreferencesRepository>>();
    let mut preferences = use_preferences();
    let mut error = use_signal(|| None::<String>);

    let on_click = move |_| {
        let snapshot = preferences.peek().clone();
        let next_prefs = Preferences {
            key_bindings: snapshot.key_bindings,
            ..Preferences::default()
        };
        match repo.save(&next_prefs) {
            Ok(()) => {
                preferences.set(next_prefs);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    rsx! {
        fieldset { class: "fieldset",
            legend { class: "fieldset-legend", "リセット" }
            button {
                r#type: "button",
                class: "btn btn-sm btn-outline btn-warning w-fit",
                onclick: on_click,
                "テーマ等をデフォルトに戻す"
            }
            p { class: "text-xs text-base-content/60 mt-1",
                "キーボードショートカット以外のすべての設定を初期値に戻します。"
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        }
    }
}
