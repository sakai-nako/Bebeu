use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::level::{Level, LevelRepository, use_levels_refresh};

#[component]
pub fn CreateLevelButton() -> Element {
    let mut show_modal = use_signal(|| false);

    rsx! {
        button {
            class: "btn btn-primary btn-sm",
            onclick: move |_| show_modal.set(true),
            "+ New"
        }

        if show_modal() {
            CreateLevelModal { onclose: move |()| show_modal.set(false) }
        }
    }
}

#[component]
fn CreateLevelModal(onclose: EventHandler<()>) -> Element {
    let repo = use_context::<Arc<dyn LevelRepository>>();
    let mut refresh = use_levels_refresh();

    let mut name = use_signal(String::new);
    let mut base_image_source = use_signal(|| None::<PathBuf>);
    let mut error = use_signal(|| None::<String>);

    let on_pick_image = move |_| {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Base 画像を選択")
            .add_filter("画像", &["png", "jpg", "jpeg", "webp", "bmp"])
            .pick_file()
        {
            base_image_source.set(Some(path));
        }
    };

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let level_name = name();
        let trimmed = level_name.trim().to_string();
        if trimmed.is_empty() {
            error.set(Some("Name は必須です".into()));
            return;
        }

        match repo.exists(&trimmed) {
            Ok(true) => {
                error.set(Some(format!("Level '{trimmed}' は既に存在します")));
                return;
            }
            Err(e) => {
                error.set(Some(e.to_string()));
                return;
            }
            Ok(false) => {}
        }

        // 画像が選ばれていれば先にコピーする。create 失敗時のロールバックのため basename を覚えておく。
        let mut imported: Option<String> = None;
        let mut new_level = Level::with_defaults(&trimmed);
        if let Some(source) = base_image_source() {
            match repo.import_base_image(&trimmed, &source) {
                Ok(basename) => {
                    imported = Some(basename.clone());
                    new_level.base = basename;
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                    return;
                }
            }
        }

        match repo.create(&new_level) {
            Ok(()) => {
                refresh.bump();
                onclose.call(());
            }
            Err(e) => {
                if let Some(basename) = imported {
                    let _ = repo.delete_base_image(&trimmed, &basename);
                }
                error.set(Some(e.to_string()));
            }
        }
    };

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "新規 Level" }

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
                            oninput: move |e| name.set(e.value()),
                        }
                        p { class: "text-xs text-base-content/60 mt-1",
                            "ファイル名 (workspace/data/levels/{{name}}.yml) になります。"
                        }
                    }

                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "Base 画像 (任意)" }

                        div { class: "flex items-center gap-2",
                            button {
                                r#type: "button",
                                class: "btn btn-outline btn-sm",
                                onclick: on_pick_image,
                                "画像を選択..."
                            }
                            if let Some(path) = base_image_source().as_ref() {
                                span { class: "text-sm font-mono",
                                    "{path.file_name().unwrap_or_default().to_string_lossy()}"
                                }
                                button {
                                    r#type: "button",
                                    class: "btn btn-ghost btn-xs",
                                    onclick: move |_| base_image_source.set(None),
                                    "クリア"
                                }
                            } else {
                                span { class: "text-sm text-base-content/60", "未選択" }
                            }
                        }
                        p { class: "text-xs text-base-content/60 mt-1",
                            "選択した画像は workspace/data/levels/{{name}}/base.{{ext}} にコピーされます。"
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
