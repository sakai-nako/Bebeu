use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::project::{ProjectRepository, use_projects_refresh};
use crate::features::project::{CreateProjectButton, DeleteProjectButton};

/// Project 一覧ページ全体を描画する。
///
/// 一覧テーブル + 「+ New Project」ボタン + 各行に Edit / Delete アクション。
#[component]
pub fn ProjectList() -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();
    let refresh = use_projects_refresh();
    let mut names = use_signal(Vec::<String>::new);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        let _ = refresh.subscribe();
        match repo.list() {
            Ok(list) => {
                names.set(list);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    });

    rsx! {
        div { class: "max-w-3xl mx-auto p-6 space-y-4",
            div { class: "flex items-center justify-between",
                h1 { class: "text-2xl font-bold", "Projects" }
                CreateProjectButton {}
            }

            if let Some(message) = error() {
                div { role: "alert", class: "alert alert-error",
                    span { "{message}" }
                }
            }

            if names().is_empty() && error().is_none() {
                div { class: "text-base-content/60 italic",
                    "Project はまだありません。"
                    "「+ New Project」で作成してください。"
                }
            } else {
                div { class: "overflow-x-auto",
                    table { class: "table table-zebra",
                        thead {
                            tr {
                                th { "Name" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            for name in names() {
                                ProjectRow { key: "{name}", name: name.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ProjectRow(name: String) -> Element {
    let nav = use_navigator();
    let target = use_signal(|| name.clone());

    let detail_path = format!("/projects/{name}");
    let row_name = name.clone();

    rsx! {
        tr {
            td {
                button {
                    r#type: "button",
                    class: "link link-primary",
                    onclick: move |_| {
                        nav.push(detail_path.clone());
                    },
                    "{row_name}"
                }
            }
            td { class: "text-right",
                div { class: "flex gap-2 justify-end",
                    DeleteProjectButton { target, ondeleted: |()| {} }
                }
            }
        }
    }
}
