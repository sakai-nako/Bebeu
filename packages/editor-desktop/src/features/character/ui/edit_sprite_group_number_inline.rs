use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{
    Character, CharacterRepository, SpriteGroup, use_characters_refresh,
};

/// SpriteGroup の Number を inline 編集する。
/// `editing` Signal は親に共有し、編集中は親側で同行の他ボタン (Rename / Delete 等) を
/// 隠せるようにする。
#[component]
pub fn EditSpriteGroupNumberInline(
    character: Character,
    sprite_group: SpriteGroup,
    mut editing: Signal<bool>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let mut draft = use_signal(|| sprite_group.number.to_string());
    let mut error = use_signal(|| None::<String>);

    let original_character = character.clone();
    let original_group = sprite_group.clone();

    let on_edit = {
        let original_group = original_group.clone();
        move |_| {
            draft.set(original_group.number.to_string());
            error.set(None);
            editing.set(true);
        }
    };

    let on_save = {
        let original_character = original_character.clone();
        let original_group = original_group.clone();
        move |_| {
            let Ok(new_number) = draft().trim().parse::<u32>() else {
                error.set(Some("Number は 0 以上の整数で入力してください".into()));
                return;
            };
            if new_number == original_group.number {
                editing.set(false);
                error.set(None);
                return;
            }
            // 同じ Character 内の他 SpriteGroup と number が衝突しないこと
            if original_character
                .sprite_groups
                .iter()
                .any(|g| g.name != original_group.name && g.number == new_number)
            {
                error.set(Some(format!("Number {new_number} は既に使われています")));
                return;
            }

            let mut updated = original_character.clone();
            if let Some(group) = updated
                .sprite_groups
                .iter_mut()
                .find(|g| g.name == original_group.name)
            {
                group.number = new_number;
            }

            match repo.update(&updated) {
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
                span { "{original_group.number}" }
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
