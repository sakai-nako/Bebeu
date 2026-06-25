use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::project::{Project, ProjectRepository, Resolution, use_projects_refresh};

/// プロジェクト名として許可する文字: 英数字・ハイフン・アンダースコア。
/// ファイル名 / パス安全性のため記号や空白は除外する。
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[component]
pub fn CreateProjectButton() -> Element {
    let mut show_modal = use_signal(|| false);

    rsx! {
        button {
            class: "btn btn-primary btn-sm",
            onclick: move |_| show_modal.set(true),
            "+ New Project"
        }

        if show_modal() {
            CreateProjectModal { onclose: move |()| show_modal.set(false) }
        }
    }
}

#[component]
fn CreateProjectModal(onclose: EventHandler<()>) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();
    let mut refresh = use_projects_refresh();

    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let on_submit = {
        let repo = repo.clone();
        move |evt: FormEvent| {
            evt.prevent_default();
            let raw = name();
            let trimmed = raw.trim().to_string();
            if !is_valid_name(&trimmed) {
                error.set(Some("Name は英数字 / - / _ のみ、空白不可".into()));
                return;
            }
            // 重複チェック
            if repo.get(&trimmed).is_ok() {
                error.set(Some(format!("Project '{trimmed}' は既に存在します")));
                return;
            }
            let new_proj = Project {
                name: trimmed,
                resolution: Resolution::default(),
                players: Vec::new(),
                opponents: Vec::new(),
                levels: Vec::new(),
                ..Project::default()
            };
            match repo.create(&new_proj) {
                Ok(()) => {
                    refresh.bump();
                    onclose.call(());
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "新規 Project" }

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
                            placeholder: "my-project",
                            oninput: move |e| name.set(e.value()),
                        }
                        p { class: "text-xs text-base-content/60 mt-1",
                            "ファイル名: workspace/data/projects/{name}.yml"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_names_accepted() {
        assert!(is_valid_name("my-project"));
        assert!(is_valid_name("Default"));
        assert!(is_valid_name("p_01"));
        assert!(is_valid_name("a"));
    }

    #[test]
    fn invalid_names_rejected() {
        assert!(!is_valid_name(""));
        assert!(!is_valid_name(" "));
        assert!(!is_valid_name("with space"));
        assert!(!is_valid_name("path/segment"));
        assert!(!is_valid_name("dot.in.middle"));
        assert!(!is_valid_name("日本語"));
    }
}
