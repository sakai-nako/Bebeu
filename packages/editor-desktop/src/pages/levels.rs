use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::level::{Level, LevelRepository, use_levels_refresh};
use crate::widgets::level::LevelDetail;

#[component]
pub fn LevelsIndex() -> Element {
    rsx! {
        div { class: "h-full flex items-center justify-center text-base-content/50",
            "サイドバーから Level を選択してください。"
        }
    }
}

#[component]
pub fn LevelDetailPage(name: ReadSignal<String>) -> Element {
    let repo = use_context::<Arc<dyn LevelRepository>>();
    let refresh = use_levels_refresh();
    let mut level = use_signal(|| None::<Level>);
    let mut not_found = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        let _ = refresh.subscribe();
        match repo.get(&name()) {
            Ok(Some(found)) => {
                level.set(Some(found));
                not_found.set(false);
                error.set(None);
            }
            Ok(None) => {
                level.set(None);
                not_found.set(true);
                error.set(None);
            }
            Err(e) => {
                level.set(None);
                not_found.set(false);
                error.set(Some(e.to_string()));
            }
        }
    });

    rsx! {
        if let Some(message) = error() {
            div { role: "alert", class: "alert alert-error",
                span { "{message}" }
            }
        } else if not_found() {
            div { class: "text-base-content/60 italic", "Level '{name}' が見つかりません。" }
        } else if let Some(lvl) = level() {
            LevelDetail { level: lvl }
        } else {
            div { class: "loading loading-spinner" }
        }
    }
}
