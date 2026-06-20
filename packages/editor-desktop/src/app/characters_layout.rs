use std::sync::Arc;

use dioxus::prelude::*;

use super::routes::Routes;
use crate::entities::character::{Character, CharacterRepository, use_characters_refresh};
use crate::widgets::character::CharactersSidebar;

#[component]
pub fn CharactersLayout() -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut characters = use_signal(Vec::<Character>::new);
    let mut error = use_signal(|| None::<String>);
    let current = use_route::<Routes>();
    let refresh = use_characters_refresh();

    use_effect(move || {
        // refresh トリガーに subscribe して、bump() のたびに再フェッチ
        let _ = refresh.subscribe();
        match repo.list() {
            Ok(list) => {
                characters.set(list);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    });

    let active_name = match &current {
        Routes::CharacterDetailPage { name }
        | Routes::SpriteGroupEditorPage { name, .. }
        | Routes::AnimationEditorPage { name, .. }
        | Routes::SoundGroupEditorPage { name, .. } => Some(name.clone()),
        // CharactersLayout は /characters 系でしか実体がレンダーされないが、Routes 網羅のため。
        Routes::CharactersIndex {}
        | Routes::LevelsIndex {}
        | Routes::LevelDetailPage { .. }
        | Routes::ProjectsIndex {}
        | Routes::ProjectDetailPage { .. } => None,
    };

    rsx! {
        div { class: "flex h-full",
            aside { class: "w-72 bg-base-200 overflow-y-auto p-4 shrink-0",
                CharactersSidebar {
                    characters: characters(),
                    error: error(),
                    active_name,
                }
            }
            main { class: "flex-1 overflow-y-auto p-6", Outlet::<Routes> {} }
        }
    }
}
