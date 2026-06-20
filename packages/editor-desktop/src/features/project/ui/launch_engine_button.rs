use dioxus::prelude::*;

use crate::entities::project::Project;

/// 指定 Project で engine を起動するためのコマンドを表示するボタン。
///
/// プロセス起動は scope 外。`just engine-run --project <name>` のコマンドを表示し、
/// ユーザーがコピーして PowerShell 等で叩く想定。Phase 2 以降で `Command::new` による
/// 自動 spawn に拡張できる余地を残している。
///
/// `workspace_dir` は engine 側の `--workspace` flag に渡す絶対パス。
#[component]
pub fn LaunchEngineButton(project: Signal<Project>, workspace_dir: ReadSignal<String>) -> Element {
    let mut show_modal = use_signal(|| false);

    let validation = use_memo(move || validate(&project.read()));
    let disabled = validation().is_err();

    rsx! {
        div { class: "flex flex-col gap-1",
            button {
                r#type: "button",
                class: "btn btn-primary btn-sm",
                disabled,
                onclick: move |_| show_modal.set(true),
                "engine 起動コマンドを表示"
            }
            if let Err(message) = validation() {
                p { class: "text-error text-xs", "{message}" }
            }
        }

        if show_modal() {
            LaunchCommandModal {
                project,
                workspace_dir,
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

#[component]
fn LaunchCommandModal(
    project: Signal<Project>,
    workspace_dir: ReadSignal<String>,
    onclose: EventHandler<()>,
) -> Element {
    let project_name = project.read().name.clone();
    let workspace = workspace_dir();

    let just_command = format!("just engine-run -- --project {project_name}");
    let raw_command =
        format!("go run ./cmd/beatemup --workspace \"{workspace}\" --project {project_name}");
    let copy_just = just_command.clone();
    let copy_raw = raw_command.clone();

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box max-w-2xl",
                h3 { class: "text-lg font-bold mb-4", "engine 起動コマンド" }

                p { class: "text-sm text-base-content/70 mb-3",
                    "以下のコマンドを PowerShell / シェルで実行すると engine が Project '{project_name}' で起動します。"
                }

                div { class: "space-y-3",
                    div {
                        p { class: "text-xs font-bold mb-1", "リポジトリルートで実行:" }
                        div { class: "flex gap-2 items-start",
                            code { class: "flex-1 bg-base-200 p-2 rounded text-xs break-all",
                                "{just_command}"
                            }
                            CopyButton { text: copy_just }
                        }
                    }
                    div {
                        p { class: "text-xs font-bold mb-1", "packages/engine/ で直接実行:" }
                        div { class: "flex gap-2 items-start",
                            code { class: "flex-1 bg-base-200 p-2 rounded text-xs break-all",
                                "{raw_command}"
                            }
                            CopyButton { text: copy_raw }
                        }
                    }
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

#[component]
fn CopyButton(text: String) -> Element {
    let copied = use_signal(|| false);
    let on_click = {
        let text = text.clone();
        let mut copied = copied;
        move |_| {
            // navigator.clipboard.writeText(...) を JS で呼ぶ
            let escaped = text.replace('\\', "\\\\").replace('`', "\\`");
            document::eval(&format!(
                "navigator.clipboard && navigator.clipboard.writeText(`{escaped}`)"
            ));
            copied.set(true);
        }
    };
    rsx! {
        button {
            r#type: "button",
            class: "btn btn-ghost btn-xs",
            onclick: on_click,
            if copied() {
                "Copied"
            } else {
                "Copy"
            }
        }
    }
}

/// engine 起動に必要な Project 設定が揃っているか確認する純関数。
///
/// players / opponents / levels がいずれか空ならエラーを返す
/// (engine 側で `[0]` アクセスが panic するため事前に弾く)。
fn validate(project: &Project) -> Result<(), String> {
    if project.players.is_empty() {
        return Err("players が空です。1 つ以上選択してください。".into());
    }
    if project.opponents.is_empty() {
        return Err("opponents が空です。1 つ以上選択してください。".into());
    }
    if project.levels.is_empty() {
        return Err("levels が空です。1 つ以上選択してください。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::project::Resolution;

    fn make(players: &[&str], opponents: &[&str], levels: &[&str]) -> Project {
        Project {
            name: "test".to_string(),
            resolution: Resolution::default(),
            players: players.iter().copied().map(String::from).collect(),
            opponents: opponents.iter().copied().map(String::from).collect(),
            levels: levels.iter().copied().map(String::from).collect(),
        }
    }

    #[test]
    fn validate_rejects_empty_players() {
        assert!(validate(&make(&[], &["B"], &["ct"])).is_err());
    }

    #[test]
    fn validate_rejects_empty_opponents() {
        assert!(validate(&make(&["A"], &[], &["ct"])).is_err());
    }

    #[test]
    fn validate_rejects_empty_levels() {
        assert!(validate(&make(&["A"], &["B"], &[])).is_err());
    }

    #[test]
    fn validate_accepts_complete_project() {
        assert!(validate(&make(&["A"], &["B"], &["ct"])).is_ok());
    }
}
