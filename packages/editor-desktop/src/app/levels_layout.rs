use std::sync::Arc;

use dioxus::prelude::*;

use super::routes::Routes;
use crate::entities::level::{LevelRepository, use_levels_refresh};
use crate::widgets::level::LevelsSidebar;

#[component]
pub fn LevelsLayout() -> Element {
    let repo = use_context::<Arc<dyn LevelRepository>>();
    let mut names = use_signal(Vec::<String>::new);
    let mut error = use_signal(|| None::<String>);
    let current = use_route::<Routes>();
    let refresh = use_levels_refresh();

    use_effect(move || {
        // refresh トリガーに subscribe して、bump() のたびに再フェッチ
        let _ = refresh.subscribe();
        match repo.list() {
            Ok(list) => {
                names.set(list);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    });

    let active_name = match &current {
        Routes::LevelDetailPage { name } => Some(name.clone()),
        Routes::LevelsIndex {}
        | Routes::CharactersIndex {}
        | Routes::CharacterDetailPage { .. }
        | Routes::SpriteGroupEditorPage { .. }
        | Routes::AnimationEditorPage { .. }
        | Routes::SoundGroupEditorPage { .. }
        | Routes::ProjectsIndex {}
        | Routes::ProjectDetailPage { .. } => None,
    };

    rsx! {
        div { class: "flex h-full",
            aside { class: "w-72 bg-base-200 overflow-y-auto p-4 shrink-0",
                LevelsSidebar { names: names(), error: error(), active_name }
            }
            main { class: "flex-1 overflow-y-auto p-6", Outlet::<Routes> {} }
        }
    }
}
