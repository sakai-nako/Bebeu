use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{Character, CharacterRepository, use_characters_refresh};

/// HP の inline 編集コンポーネント。表示モード ↔ 編集モードを切り替える。
#[component]
pub fn EditHpInline(character: Character) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let mut editing = use_signal(|| false);
    let mut draft = use_signal(|| character.hp.to_string());
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_edit = {
        let original = original.clone();
        move |_| {
            draft.set(original.hp.to_string());
            error.set(None);
            editing.set(true);
        }
    };

    let on_save = {
        let original = original.clone();
        move |_| {
            let Ok(new_hp) = draft().trim().parse::<u32>() else {
                error.set(Some("HP は 0 以上の整数で入力してください".into()));
                return;
            };
            let updated = Character {
                hp: new_hp,
                ..original.clone()
            };
            match repo.update_metadata(&updated) {
                Ok(()) => {
                    refresh.bump();
                    editing.set(false);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    let on_cancel = move |_| {
        editing.set(false);
        error.set(None);
    };

    rsx! {
        if editing() {
            div { class: "flex items-center gap-2",
                input {
                    r#type: "number",
                    class: "input input-bordered input-sm w-24",
                    value: "{draft}",
                    min: "0",
                    oninput: move |e| draft.set(e.value()),
                }
                button {
                    r#type: "button",
                    class: "btn btn-primary btn-xs",
                    onclick: on_save,
                    "Save"
                }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: on_cancel,
                    "Cancel"
                }
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        } else {
            div { class: "flex items-center gap-2",
                span { "{original.hp}" }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: on_edit,
                    title: "編集",
                    "✎"
                }
            }
        }
    }
}
