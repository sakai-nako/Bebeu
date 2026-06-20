use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{CharacterRepository, use_characters_refresh};

#[component]
pub fn DeleteAnimationButton(character_name: String, animation_name: String) -> Element {
    let mut show_modal = use_signal(|| false);
    let modal_character_name = character_name.clone();
    let modal_animation_name = animation_name.clone();

    rsx! {
        button {
            class: "btn btn-error btn-outline btn-sm",
            onclick: move |_| show_modal.set(true),
            "Delete"
        }

        if show_modal() {
            DeleteAnimationModal {
                character_name: modal_character_name.clone(),
                animation_name: modal_animation_name.clone(),
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

#[component]
fn DeleteAnimationModal(
    character_name: String,
    animation_name: String,
    onclose: EventHandler<()>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();
    let nav = use_navigator();
    let mut error = use_signal(|| None::<String>);

    let target_character = character_name.clone();
    let target_animation = animation_name.clone();

    let on_confirm = move |_| match repo.delete_animation(&target_character, &target_animation) {
        Ok(()) => {
            refresh.bump();
            onclose.call(());
            nav.replace(format!("/characters/{target_character}"));
        }
        Err(e) => error.set(Some(e.to_string())),
    };

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-2", "Animation を削除" }
                p { class: "py-2",
                    "'"
                    span { class: "font-mono font-semibold", "{animation_name}" }
                    "' を削除しますか？この Animation の frames も全て削除されます。"
                }

                if let Some(message) = error() {
                    div { role: "alert", class: "alert alert-error mt-2",
                        span { "{message}" }
                    }
                }

                div { class: "modal-action",
                    button {
                        class: "btn btn-ghost",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button { class: "btn btn-error", onclick: on_confirm, "削除" }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}
