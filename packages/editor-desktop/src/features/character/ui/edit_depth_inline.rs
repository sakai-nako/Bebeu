use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{Character, CharacterRepository, use_characters_refresh};

/// Depth (world Z 厚み) の inline 編集コンポーネント。`EditHpInline` と同じ Pattern で
/// 表示モード ↔ 編集モードを切り替える。
///
/// HitBox.depth が None の box はこの値にフォールバックする (ADR-0024)。
/// 値は u32 として受け、ゼロでも許容する (= 厚みゼロで原理的に当たらない、特殊ケース)。
#[component]
pub fn EditDepthInline(character: Character) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let mut editing = use_signal(|| false);
    let mut draft = use_signal(|| character.depth.to_string());
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_edit = {
        let original = original.clone();
        move |_| {
            draft.set(original.depth.to_string());
            error.set(None);
            editing.set(true);
        }
    };

    let on_save = {
        let original = original.clone();
        move |_| {
            let Ok(new_depth) = draft().trim().parse::<u32>() else {
                error.set(Some("Depth は 0 以上の整数で入力してください".into()));
                return;
            };
            let updated = Character {
                depth: new_depth,
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
                span { "{original.depth}" }
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
