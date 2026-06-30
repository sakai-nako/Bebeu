use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{
    Character, CharacterPhysics, CharacterRepository, DEFAULT_CHARACTER_DEPTH, Sprite, SpriteGroup,
    use_characters_refresh,
};

const DEFAULT_HP: u32 = 100;
const DEFAULT_SPRITE_GROUP_NAME: &str = "thumbnail";
const DEFAULT_SPRITE_GROUP_NUMBER: u32 = 10000;

#[component]
pub fn CreateCharacterButton() -> Element {
    let mut show_modal = use_signal(|| false);

    rsx! {
        button {
            class: "btn btn-primary btn-sm",
            onclick: move |_| show_modal.set(true),
            "+ New"
        }

        if show_modal() {
            CreateCharacterModal { onclose: move |()| show_modal.set(false) }
        }
    }
}

#[component]
fn CreateCharacterModal(onclose: EventHandler<()>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let mut name = use_signal(String::new);
    let mut thumbnail_image_source = use_signal(|| None::<PathBuf>);
    let mut sprite_group_name = use_signal(|| String::from(DEFAULT_SPRITE_GROUP_NAME));
    // 数値入力は表示を String で持ち、submit 時にパースする
    // （u32 に直バインドすると "400" → "00" に編集する途中で 0 に丸まり、表示が壊れる）
    let mut sprite_group_number_input = use_signal(|| DEFAULT_SPRITE_GROUP_NUMBER.to_string());
    let mut error = use_signal(|| None::<String>);

    let on_pick_image = move |_| {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("サムネイル画像を選択")
            .add_filter("画像", &["png", "jpg", "jpeg", "gif", "webp", "svg"])
            .pick_file()
        {
            thumbnail_image_source.set(Some(path));
        }
    };

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let char_name = name();
        if char_name.trim().is_empty() {
            error.set(Some("Name は必須です".into()));
            return;
        }

        // 重複チェック（事前）
        match repo.get(&char_name) {
            Ok(Some(_)) => {
                error.set(Some(format!("Character '{char_name}' は既に存在します")));
                return;
            }
            Err(e) => {
                error.set(Some(e.to_string()));
                return;
            }
            Ok(None) => {}
        }

        // サムネイル画像が選ばれていれば取り込む。クリーンアップに備えて (group_name, basename) を覚えておく
        let mut imported: Option<(String, String)> = None;
        let (thumbnail_path, sprite_groups) = if let Some(source) = thumbnail_image_source() {
            let group_name = sprite_group_name();
            if group_name.trim().is_empty() {
                error.set(Some("Sprite Group Name は必須です".into()));
                return;
            }
            let Ok(group_number) = sprite_group_number_input().trim().parse::<u32>() else {
                error.set(Some(
                    "Sprite Group Number は 0 以上の整数で入力してください".into(),
                ));
                return;
            };
            // import 前の元 PNG から dimensions を読んでおく (4K 描画の explicit sizing 用)。
            // 失敗した場合 (壊れた PNG / 別形式) は None で、次回 loader 経由 reload で埋まる。
            let dims = crate::shared::read_png_dimensions(&source).ok();
            let basename = match repo.import_sprite_image(&char_name, &group_name, &source) {
                Ok(b) => b,
                Err(e) => {
                    error.set(Some(e.to_string()));
                    return;
                }
            };
            imported = Some((group_name.clone(), basename.clone()));

            let sprite = Sprite {
                index: 0,
                path: basename.clone(),
                pivot_point: [0, 0],
                body_boxes: None,
                attack_boxes: None,
                dimensions: dims,
            };
            let group = SpriteGroup {
                name: group_name.clone(),
                number: group_number,
                sprites: vec![sprite],
            };
            let path = format!("sprite-groups/{group_name}/sprites/{basename}");
            (path, vec![group])
        } else {
            (String::new(), Vec::new())
        };

        let new_char = Character {
            name: char_name.clone(),
            thumbnail_path,
            hp: DEFAULT_HP,
            depth: DEFAULT_CHARACTER_DEPTH,
            tag: None,
            physics: CharacterPhysics::default(),
            ai: None,
            sprite_groups,
            animations: Vec::new(),
            sound_groups: Vec::new(),
        };

        match repo.create(&new_char) {
            Ok(()) => {
                refresh.bump();
                onclose.call(());
            }
            Err(e) => {
                // 画像をコピー済みならロールバック
                if let Some((group_name, basename)) = imported {
                    let _ = repo.delete_sprite_image(&char_name, &group_name, &basename);
                }
                error.set(Some(e.to_string()));
            }
        }
    };

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "新規 Character" }

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
                    }

                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "Thumbnail (任意)" }

                        div { class: "flex items-center gap-2",
                            button {
                                r#type: "button",
                                class: "btn btn-outline btn-sm",
                                onclick: on_pick_image,
                                "画像を選択..."
                            }
                            if let Some(path) = thumbnail_image_source().as_ref() {
                                span { class: "text-sm font-mono",
                                    "{path.file_name().unwrap_or_default().to_string_lossy()}"
                                }
                                button {
                                    r#type: "button",
                                    class: "btn btn-ghost btn-xs",
                                    onclick: move |_| thumbnail_image_source.set(None),
                                    "クリア"
                                }
                            } else {
                                span { class: "text-sm text-base-content/60", "未選択" }
                            }
                        }

                        if thumbnail_image_source().is_some() {
                            div { class: "mt-3 pl-3 border-l-2 border-base-300 space-y-2",
                                p { class: "text-sm text-base-content/70",
                                    "選択した画像を Sprite として含む SpriteGroup を新規作成します"
                                }
                                fieldset { class: "fieldset",
                                    legend { class: "fieldset-legend", "Sprite Group Name" }
                                    input {
                                        class: "input input-bordered input-sm w-full",
                                        value: "{sprite_group_name}",
                                        spellcheck: "false",
                                        oninput: move |e| sprite_group_name.set(e.value()),
                                    }
                                }
                                fieldset { class: "fieldset",
                                    legend { class: "fieldset-legend", "Sprite Group Number" }
                                    input {
                                        r#type: "number",
                                        class: "input input-bordered input-sm w-full",
                                        value: "{sprite_group_number_input}",
                                        min: "0",
                                        oninput: move |e| sprite_group_number_input.set(e.value()),
                                    }
                                }
                            }
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
