use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{
    Animation, Character, CharacterRepository, use_characters_refresh, use_playback,
};
use crate::entities::keybinding::Action;
use crate::entities::navigation_guard::use_navigation_guard;
use crate::features::keybinding::use_keyboard_action;
use crate::shared::UseHistory;

#[component]
pub fn AnimationEditorActions(
    character: Character,
    original_animation: Animation,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    mut baseline: Signal<Animation>,
    mut selected_frame_index: Signal<usize>,
    mut selected_layer_index: Signal<Option<usize>>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();
    let nav = use_navigator();
    let mut guard = use_navigation_guard();
    let mut error = use_signal(|| None::<String>);
    // 再生中は編集系 (Save / Undo / Redo / Frame nav) のショートカットとボタンをロックする
    let playback = use_playback();
    let locked = playback().locks_editing();

    // is_dirty を NavigationGuard に同期する。Cancel/breadcrumb/左 rail どこから離脱しても確認が出る。
    use_effect(move || {
        let dirty = draft() != *baseline.read();
        guard.set_blocked(dirty);
    });

    // unmount 時には blocked を解除する（ダイアログが残らないように）
    use_drop(move || {
        guard.set_blocked(false);
    });

    // Cancel ボタンの遷移先。Detail を廃止したので、Cancel は Character ページへ戻す。
    let cancel_url = format!("/characters/{}", character.name);
    // original_animation は Save 後の遷移ロジック等で使うため、未使用警告を避けるためにここで参照に留める
    let _ = &original_animation;

    let on_save = use_callback({
        let character_name = character.name.clone();
        let repo = repo.clone();
        move |()| {
            let current_draft = draft();
            match repo.update_animation(&character_name, &current_draft) {
                Ok(()) => {
                    // 保存後は編集画面に留まる。draft / baseline を揃えて dirty 判定をリセット。
                    baseline.set(current_draft);
                    refresh.bump();
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    });

    use_keyboard_action(Action::Save, move || {
        if playback.peek().locks_editing() {
            return;
        }
        on_save.call(());
    });
    use_keyboard_action(Action::Undo, move || {
        if playback.peek().locks_editing() {
            return;
        }
        let mut h = history;
        h.undo();
    });
    use_keyboard_action(Action::Redo, move || {
        if playback.peek().locks_editing() {
            return;
        }
        let mut h = history;
        h.redo();
    });

    // Frame 選択: 現在の index を 1 つ進める / 戻す / 端へジャンプする。
    // frames が空のときは何もしない。Layer 選択は対象 Frame が変わるたびにクリア。
    let select_frame = use_callback(move |new_index: usize| {
        if playback.peek().locks_editing() {
            return;
        }
        let len = draft.peek().frames.len();
        if len == 0 {
            return;
        }
        let clamped = new_index.min(len - 1);
        if *selected_frame_index.peek() != clamped {
            selected_frame_index.set(clamped);
            selected_layer_index.set(None);
        }
    });
    use_keyboard_action(Action::SelectPrevFrame, move || {
        let current = *selected_frame_index.peek();
        if current > 0 {
            select_frame.call(current - 1);
        }
    });
    use_keyboard_action(Action::SelectNextFrame, move || {
        let current = *selected_frame_index.peek();
        select_frame.call(current + 1);
    });
    use_keyboard_action(Action::SelectFirstFrame, move || {
        select_frame.call(0);
    });
    use_keyboard_action(Action::SelectLastFrame, move || {
        let len = draft.peek().frames.len();
        if len > 0 {
            select_frame.call(len - 1);
        }
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
                disabled: !can_undo || locked,
                title: "元に戻す",
                onclick: move |_| {
                    let mut h = history;
                    h.undo();
                },
                "↶ Undo"
            }
            button {
                class: "btn btn-ghost btn-sm",
                disabled: !can_redo || locked,
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
                disabled: locked,
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
