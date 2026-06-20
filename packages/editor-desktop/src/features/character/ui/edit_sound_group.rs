use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{
    Character, CharacterRepository, SoundGroup, use_characters_refresh,
};
use crate::entities::keybinding::Action;
use crate::entities::navigation_guard::use_navigation_guard;
use crate::features::keybinding::use_keyboard_action;
use crate::shared::UseHistory;

/// SoundGroupEditor 上段のアクション (Save / Cancel / Undo / Redo)。
///
/// 設計は `AnimationEditorActions` を踏襲しているが、SoundGroupEditor では Frame 選択や
/// 再生のような状態が無いのでシグネチャは縮小されている。Save 成功時に
/// `pending_imports` を空にして `SoundGroupEditor` の `use_drop` による wav rollback を抑止する。
#[component]
pub fn SoundGroupEditorActions(
    character: Character,
    original_group: SoundGroup,
    mut draft: Signal<SoundGroup>,
    mut history: UseHistory<SoundGroup>,
    mut baseline: Signal<SoundGroup>,
    mut pending_imports: Signal<Vec<String>>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();
    let nav = use_navigator();
    let mut guard = use_navigation_guard();
    let mut error = use_signal(|| None::<String>);

    // is_dirty を NavigationGuard に同期する。Cancel/breadcrumb どこから離脱しても確認が出る。
    use_effect(move || {
        let dirty = draft() != *baseline.read();
        guard.set_blocked(dirty);
    });

    // unmount 時には blocked を解除する（ダイアログが残らないように）
    use_drop(move || {
        guard.set_blocked(false);
    });

    let cancel_url = format!("/characters/{}", character.name);
    // original_group は将来の遷移ロジックで使うため、未使用警告を避けるためここで参照に留める
    let _ = &original_group;

    let on_save = use_callback({
        let character_name = character.name.clone();
        let repo = repo.clone();
        move |()| {
            let current_draft = draft();
            match repo.update_sound_group(&character_name, &current_draft) {
                Ok(()) => {
                    // 保存後は編集画面に留まる。draft / baseline を揃えて dirty 判定をリセット。
                    baseline.set(current_draft);
                    // pending_imports をクリアして use_drop での wav 削除を抑止 (= commit 完了)。
                    pending_imports.set(Vec::new());
                    refresh.bump();
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    });

    // Save / Undo / Redo は 3 エディタ共通の単一 Action。mount された Editor の hook だけが
    // register されるので、active な editor 1 つだけが処理する（unmount で hook ごと消える）。
    use_keyboard_action(Action::Save, move || on_save.call(()));
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
