use dioxus::prelude::*;

use crate::entities::preference::use_t;
use crate::features::keybinding::EditKeyBindings;
use crate::features::preference::{
    ChangeLocaleSelect, ChangeThemeSelect, EditHistoryCapacity, ResetPreferencesButton,
};

#[component]
pub fn PreferencesModal(onclose: EventHandler<()>) -> Element {
    let t = use_t();
    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "{t(\"preferences.title\")}" }

                div { class: "space-y-4",
                    ChangeLocaleSelect {}
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
                        "{t(\"preferences.close\")}"
                    }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}
