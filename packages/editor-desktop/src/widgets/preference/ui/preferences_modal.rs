use dioxus::prelude::*;

use crate::features::keybinding::EditKeyBindings;
use crate::features::preference::{ChangeThemeSelect, EditHistoryCapacity, ResetPreferencesButton};

#[component]
pub fn PreferencesModal(onclose: EventHandler<()>) -> Element {
    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "Preferences" }

                div { class: "space-y-4",
                    ChangeThemeSelect {}
                    EditHistoryCapacity {}
                    ResetPreferencesButton {}
                    EditKeyBindings {}
                }

                div { class: "modal-action",
                    button {
                        r#type: "button",
                        class: "btn btn-primary",
                        onclick: move |_| onclose.call(()),
                        "閉じる"
                    }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}
