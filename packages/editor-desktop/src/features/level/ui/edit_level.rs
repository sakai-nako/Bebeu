use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::keybinding::Action;
use crate::entities::level::{Level, LevelRepository, use_levels_refresh};
use crate::entities::navigation_guard::use_navigation_guard;
use crate::features::keybinding::use_keyboard_action;
use crate::shared::UseHistory;

/// LevelEditor の Save / Cancel / Undo / Redo を担うアクションバー (Pattern D)。
///
/// `draft` と `original` (= baseline) の比較で `is_dirty` を判定し、NavigationGuard に
/// 同期する。Save 成功時は `baseline` を draft に揃え、`refresh.bump()` で一覧 / detail を
/// 再フェッチさせる。
#[component]
pub fn LevelEditorActions(
    original: Level,
    mut draft: Signal<Level>,
    mut history: UseHistory<Level>,
) -> Element {
    let repo = use_context::<Arc<dyn LevelRepository>>();
    let mut refresh = use_levels_refresh();
    let nav = use_navigator();
    let mut guard = use_navigation_guard();
    let mut error = use_signal(|| None::<String>);
    let mut baseline = use_signal(|| original.clone());

    // is_dirty を NavigationGuard に同期。breadcrumb / 左 rail / Cancel すべての離脱経路で
    // 確認ダイアログが出るようにする。
    use_effect(move || {
        let dirty = draft() != *baseline.read();
        guard.set_blocked(dirty);
    });

    // unmount 時に blocked を解除する (異常系で離脱後も confirm が残るのを防ぐ)
    use_drop(move || {
        guard.set_blocked(false);
    });

    let cancel_url = "/levels".to_string();

    let on_save = use_callback({
        let repo = repo.clone();
        move |()| {
            let current_draft = draft();
            match repo.save(&current_draft) {
                Ok(()) => {
                    baseline.set(current_draft);
                    refresh.bump();
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    });

    // Ctrl+S で同じ保存処理を発火する
    use_keyboard_action(Action::Save, move || on_save.call(()));

    // Undo / Redo: UseHistory は Copy なので各クロージャに別コピーを渡す。
    use_keyboard_action(Action::Undo, move || {
        let mut h = history;
        h.undo();
    });
    use_keyboard_action(Action::Redo, move || {
        let mut h = history;
        h.redo();
    });

    let on_cancel_clicked = {
        let cancel_url = cancel_url.clone();
        move |_| guard.try_navigate(&nav, cancel_url.clone())
    };

    let is_dirty = draft() != *baseline.read();
    let can_undo = history.can_undo();
    let can_redo = history.can_redo();

    rsx! {
        div { class: "flex items-center gap-2",
            button {
                class: "btn btn-ghost btn-sm",
                disabled: !can_undo,
                title: "元に戻す",
                onclick: move |_| {
                    let mut h = history;
                    h.undo();
                },
                "↶ Undo"
            }
            button {
                class: "btn btn-ghost btn-sm",
                disabled: !can_redo,
                title: "やり直す",
                onclick: move |_| {
                    let mut h = history;
                    h.redo();
                },
                "↷ Redo"
            }
            div { class: "divider divider-horizontal mx-0" }
            button { class: "btn btn-ghost btn-sm", onclick: on_cancel_clicked, "Cancel" }
            if is_dirty {
                span {
                    class: "badge badge-warning badge-sm",
                    title: "未保存の変更があります",
                    "● 未保存"
                }
            }
            button {
                class: "btn btn-primary btn-sm",
                onclick: move |_| on_save.call(()),
                "Save"
            }
        }

        if let Some(message) = error() {
            div { role: "alert", class: "alert alert-error mt-2",
                span { "{message}" }
            }
        }
    }
}
