use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{
    Character, CharacterRepository, SoundGroup, use_characters_refresh,
};

#[component]
pub fn CreateSoundGroupButton(character: Character) -> Element {
    let mut show_modal = use_signal(|| false);
    let modal_character = character.clone();

    rsx! {
        button {
            class: "btn btn-primary btn-sm",
            onclick: move |_| show_modal.set(true),
            "+ New"
        }

        if show_modal() {
            CreateSoundGroupModal {
                character: modal_character.clone(),
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

#[component]
fn CreateSoundGroupModal(character: Character, onclose: EventHandler<()>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let mut name = use_signal(String::new);
    let mut number_input = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let new_name = name();
        let new_name_trimmed = new_name.trim();
        if new_name_trimmed.is_empty() {
            error.set(Some("Name は必須です".into()));
            return;
        }
        if original
            .sound_groups
            .iter()
            .any(|g| g.name == new_name_trimmed)
        {
            error.set(Some(format!(
                "SoundGroup '{new_name_trimmed}' は既に存在します"
            )));
            return;
        }
        let Ok(new_number) = number_input().trim().parse::<u32>() else {
            error.set(Some("Number は 0 以上の整数で入力してください".into()));
            return;
        };
        if original.sound_groups.iter().any(|g| g.number == new_number) {
            error.set(Some(format!("Number {new_number} は既に使われています")));
            return;
        }

        let mut updated = original.clone();
        updated.sound_groups.push(SoundGroup {
            name: new_name_trimmed.to_string(),
            number: new_number,
            sounds: Vec::new(),
        });

        match repo.update(&updated) {
            Ok(()) => {
                refresh.bump();
                onclose.call(());
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "新規 Sound Group" }

                form { class: "space-y-3", onsubmit: on_submit,
                    if let Some(message) = error() {
                        div { role: "alert", class: "alert alert-error",
                            span { "{message}" }
                        }
                    }

                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "Name" }
                        input {
                            class: "input input-bordered w-full",
                            value: "{name}",
                            required: true,
                            spellcheck: "false",
                            oninput: move |e| name.set(e.value()),
                        }
                    }

                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "Number" }
                        input {
                            r#type: "number",
                            class: "input input-bordered w-full",
                            value: "{number_input}",
                            min: "0",
                            required: true,
                            oninput: move |e| number_input.set(e.value()),
                        }
                    }

                    div { class: "modal-action",
                        button {
                            r#type: "button",
                            class: "btn btn-ghost",
                            onclick: move |_| onclose.call(()),
                            "Cancel"
                        }
                        button { r#type: "submit", class: "btn btn-primary", "Create" }
                    }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}
