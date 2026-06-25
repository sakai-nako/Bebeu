use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{Character, CharacterRepository};
use crate::entities::level::LevelRepository;
use crate::entities::project::{Project, ProjectRepository, use_projects_refresh};
use crate::features::hud::EditHudLayout;
use crate::features::project::{
    DeleteProjectButton, EditProjectResolution, LaunchEngineButton, ProjectRole,
    ProjectRoleSelector,
};

/// URL の :name で指定された Project の詳細編集ページ。
#[component]
pub fn ProjectDetail(target_name: ReadSignal<String>) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();
    let refresh = use_projects_refresh();
    let nav = use_navigator();
    let mut current_project = use_signal(Project::default);
    let mut load_error = use_signal(|| None::<String>);
    let mut loaded = use_signal(|| false);

    use_effect(move || {
        let _ = refresh.subscribe();
        let name = target_name();
        match repo.get(&name) {
            Ok(p) => {
                current_project.set(p);
                load_error.set(None);
                loaded.set(true);
            }
            Err(e) => {
                load_error.set(Some(e.to_string()));
                loaded.set(false);
            }
        }
    });

    let target_signal: Signal<String> = use_signal(|| target_name.peek().clone());
    let on_deleted = move |()| {
        nav.push("/projects".to_string());
    };

    rsx! {
        div { class: "space-y-6",
            div { class: "breadcrumbs text-sm",
                ul {
                    li { "projects" }
                    li { "{target_name}" }
                }
            }

            div { class: "flex items-center gap-3",
                h1 { class: "text-3xl font-bold", "{target_name}" }
                DeleteProjectButton { target: target_signal, ondeleted: on_deleted }
            }

            if let Some(message) = load_error() {
                div { role: "alert", class: "alert alert-error",
                    span { "{message}" }
                }
            }

            if loaded() {
                ProjectEditForm { project: current_project }
            } else if load_error().is_none() {
                div { class: "loading loading-spinner" }
            }
        }
    }
}

/// Project 1 つに対する編集フォーム本体。
#[component]
fn ProjectEditForm(project: Signal<Project>) -> Element {
    let character_repo = use_context::<Arc<dyn CharacterRepository>>();
    let level_repo = use_context::<Arc<dyn LevelRepository>>();
    let mut character_names = use_signal(Vec::<String>::new);
    let mut level_names = use_signal(Vec::<String>::new);
    let mut load_error = use_signal(|| None::<String>);

    use_effect({
        let character_repo = character_repo.clone();
        move || match character_repo.list() {
            Ok(list) => {
                let names: Vec<String> = list.into_iter().map(|c: Character| c.name).collect();
                character_names.set(names);
            }
            Err(e) => load_error.set(Some(e.to_string())),
        }
    });

    use_effect({
        let level_repo = level_repo.clone();
        move || match level_repo.list() {
            Ok(list) => level_names.set(list),
            Err(e) => load_error.set(Some(e.to_string())),
        }
    });

    let chars_signal: ReadSignal<Vec<String>> = ReadSignal::new(character_names);
    let levels_signal: ReadSignal<Vec<String>> = ReadSignal::new(level_names);

    rsx! {
        if let Some(message) = load_error() {
            div { role: "alert", class: "alert alert-error",
                span { "{message}" }
            }
        }

        section { class: "card bg-base-100 shadow-sm",
            div { class: "card-body",
                EditProjectResolution { project }
            }
        }

        div { class: "grid md:grid-cols-3 gap-4",
            section { class: "card bg-base-100 shadow-sm",
                div { class: "card-body",
                    ProjectRoleSelector {
                        role: ProjectRole::Players,
                        available: chars_signal,
                        project,
                    }
                }
            }
            section { class: "card bg-base-100 shadow-sm",
                div { class: "card-body",
                    ProjectRoleSelector {
                        role: ProjectRole::Opponents,
                        available: chars_signal,
                        project,
                    }
                }
            }
            section { class: "card bg-base-100 shadow-sm",
                div { class: "card-body",
                    ProjectRoleSelector {
                        role: ProjectRole::Levels,
                        available: levels_signal,
                        project,
                    }
                }
            }
        }

        section { class: "card bg-base-100 shadow-sm",
            div { class: "card-body",
                EditHudLayout { project }
            }
        }

        section { class: "card bg-base-100 shadow-sm",
            div { class: "card-body",
                h2 { class: "card-title text-base", "engine 起動" }
                p { class: "text-sm text-base-content/70 mb-2",
                    "この Project を指定して engine を起動します。実際のプロセス起動は手動で行ってください。"
                }
                LaunchEngineButton { project }
            }
        }
    }
}
