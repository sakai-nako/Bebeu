use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{Character, CharacterRepository, use_characters_refresh};

#[component]
pub fn RenameCharacterButton(character: Character) -> Element {
    let mut show_modal = use_signal(|| false);
    let modal_character = character.clone();

    rsx! {
        button {
            class: "btn btn-primary btn-outline btn-sm",
            onclick: move |_| show_modal.set(true),
            "Rename"
        }

        if show_modal() {
            RenameCharacterModal {
                character: modal_character.clone(),
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

#[component]
fn RenameCharacterModal(character: Character, onclose: EventHandler<()>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();
    let nav = use_navigator();

    let old_name = character.name.clone();
    let mut new_name = use_signal(|| old_name.clone());
    let mut error = use_signal(|| None::<String>);

    let on_submit = {
        let old_name = old_name.clone();
        move |evt: FormEvent| {
            evt.prevent_default();
            let new_name_value = new_name();
            let new_name_trimmed = new_name_value.trim();
            if new_name_trimmed.is_empty() {
                error.set(Some("Name は必須です".into()));
                return;
            }
            if new_name_trimmed == old_name {
                error.set(Some("名前が変更されていません".into()));
                return;
            }
            match repo.rename(&old_name, new_name_trimmed) {
                Ok(()) => {
                    refresh.bump();
                    onclose.call(());
                    // URL の旧名を新名で置き換え
                    nav.replace(format!("/characters/{new_name_trimmed}"));
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-2", "Character をリネーム" }
                p { class: "py-2 text-sm text-base-content/70",
                    "現在の名前: "
                    span { class: "font-mono font-semibold", "{old_name}" }
                }

                form { class: "space-y-3", onsubmit: on_submit,
                    if let Some(message) = error() {
                        div { role: "alert", class: "alert alert-error",
                            span { "{message}" }
                        }
                    }

                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "新しい Name" }
                        input {
                            class: "input input-bordered w-full",
                            value: "{new_name}",
                            required: true,
                            spellcheck: "false",
                            oninput: move |e| new_name.set(e.value()),
                        }
                    }

                    div { class: "modal-action",
                        button {
                            r#type: "button",
                            class: "btn btn-ghost",
                            onclick: move |_| onclose.call(()),
                            "Cancel"
                        }
                        button { r#type: "submit", class: "btn btn-primary", "Rename" }
                    }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}
