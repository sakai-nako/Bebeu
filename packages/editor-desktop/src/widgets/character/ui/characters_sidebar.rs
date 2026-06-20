use dioxus::prelude::*;

use crate::entities::character::Character;
use crate::entities::navigation_guard::use_navigation_guard;
use crate::features::character::CreateCharacterButton;

#[component]
pub fn CharactersSidebar(
    characters: Vec<Character>,
    error: Option<String>,
    active_name: Option<String>,
) -> Element {
    let has_error = error.is_some();

    rsx! {
        div { class: "space-y-3",
            div { class: "flex items-center justify-between",
                h1 { class: "text-xl font-bold", "Characters" }
                CreateCharacterButton {}
            }

            if let Some(message) = error.as_ref() {
                div { role: "alert", class: "alert alert-error",
                    span { "{message}" }
                }
            }

            if characters.is_empty() && !has_error {
                div { class: "text-base-content/60 italic text-sm",
                    "Character がまだありません。"
                }
            } else {
                nav { class: "flex flex-col gap-1",
                    for character in characters {
                        SidebarLink {
                            key: "{character.name}",
                            character_name: character.name.clone(),
                            is_active: active_name.as_deref() == Some(character.name.as_str()),
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SidebarLink(character_name: String, is_active: bool) -> Element {
    let mut guard = use_navigation_guard();
    let nav = use_navigator();
    let class = if is_active {
        "px-3 py-2 rounded bg-primary text-primary-content cursor-pointer"
    } else {
        "px-3 py-2 rounded hover:bg-base-300 cursor-pointer"
    };
    let route = format!("/characters/{character_name}");

    rsx! {
        a {
            class: "{class}",
            onclick: move |_| guard.try_navigate(&nav, route.clone()),
            "{character_name}"
        }
    }
}
