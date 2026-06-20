use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::project::{ProjectRepository, use_projects_refresh};

/// 指定された Project を削除する確認モーダル。
#[component]
pub fn DeleteProjectButton(target: ReadSignal<String>, ondeleted: EventHandler<()>) -> Element {
    let mut show_modal = use_signal(|| false);

    rsx! {
        button {
            class: "btn btn-error btn-sm",
            onclick: move |_| show_modal.set(true),
            "Delete"
        }

        if show_modal() {
            DeleteProjectConfirm {
                target,
                onclose: move |()| show_modal.set(false),
                ondeleted,
            }
        }
    }
}

#[component]
fn DeleteProjectConfirm(
    target: ReadSignal<String>,
    onclose: EventHandler<()>,
    ondeleted: EventHandler<()>,
) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();
    let mut refresh = use_projects_refresh();
    let mut error = use_signal(|| None::<String>);

    let on_confirm = {
        let repo = repo.clone();
        move |_| {
            let name = target();
            if let Err(e) = repo.delete(&name) {
                error.set(Some(e.to_string()));
                return;
            }
            refresh.bump();
            onclose.call(());
            ondeleted.call(());
        }
    };

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-2", "Project を削除しますか？" }
                p { class: "py-2",
                    "Project '{target}' の YAML ファイル (workspace/data/projects/{target}.yml) を削除します。Character / Level の master pool には影響しません。"
                }
                if let Some(message) = error() {
                    div { role: "alert", class: "alert alert-error mt-2",
                        span { "{message}" }
                    }
                }
                div { class: "modal-action",
                    button {
                        r#type: "button",
                        class: "btn btn-ghost",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-error",
                        onclick: on_confirm,
                        "削除"
                    }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}
