use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{Character, CharacterRepository, use_characters_refresh};

#[component]
pub fn ChangeThumbnailButton(character: Character) -> Element {
    let mut show_modal = use_signal(|| false);
    let modal_character = character.clone();
    let has_sprite_groups = !character.sprite_groups.is_empty();

    rsx! {
        button {
            class: "btn btn-outline btn-sm",
            disabled: !has_sprite_groups,
            title: if has_sprite_groups { "" } else { "Sprite Group がありません" },
            onclick: move |_| show_modal.set(true),
            "Change Thumbnail"
        }

        if show_modal() {
            ChangeThumbnailModal {
                character: modal_character.clone(),
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

/// 現在の `thumbnail_path` から (group_name, sprite_path) を取り出す。
/// 期待フォーマット: `sprite-groups/{group}/sprites/{filename}`
fn parse_thumbnail_path(thumbnail_path: &str) -> Option<(String, String)> {
    let rest = thumbnail_path.strip_prefix("sprite-groups/")?;
    let (group, rest) = rest.split_once('/')?;
    let sprite_path = rest.strip_prefix("sprites/")?;
    Some((group.to_string(), sprite_path.to_string()))
}

#[component]
fn ChangeThumbnailModal(character: Character, onclose: EventHandler<()>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    // 現在の thumbnail_path から初期選択を決定。マッチしなければ最初の group / sprite を default
    let (initial_group, initial_sprite_index) = parse_thumbnail_path(&character.thumbnail_path)
        .and_then(|(group, sprite_path)| {
            character.sprite_groups.iter().find_map(|sg| {
                if sg.name != group {
                    return None;
                }
                let idx = sg.sprites.iter().find(|s| s.path == sprite_path)?.index;
                Some((sg.name.clone(), idx))
            })
        })
        .unwrap_or_else(|| {
            let first_group = character.sprite_groups[0].name.clone();
            let first_index = character.sprite_groups[0]
                .sprites
                .first()
                .map_or(0, |s| s.index);
            (first_group, first_index)
        });

    let mut selected_group = use_signal(|| initial_group);
    let mut selected_sprite_index = use_signal(|| initial_sprite_index);
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_submit = {
        let original = original.clone();
        move |evt: FormEvent| {
            evt.prevent_default();
            let group_name = selected_group();
            let Some(group) = original.sprite_groups.iter().find(|g| g.name == group_name) else {
                error.set(Some("Sprite Group が見つかりません".into()));
                return;
            };
            let sprite_index = selected_sprite_index();
            let Some(sprite) = group.sprites.iter().find(|s| s.index == sprite_index) else {
                error.set(Some("Sprite が見つかりません".into()));
                return;
            };
            let new_thumbnail = format!("sprite-groups/{}/sprites/{}", group_name, sprite.path);
            let updated = Character {
                thumbnail_path: new_thumbnail,
                ..original.clone()
            };
            match repo.update_metadata(&updated) {
                Ok(()) => {
                    refresh.bump();
                    onclose.call(());
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    // 選択中 group の sprites を取り出す（描画用）
    let current_group_name = selected_group();
    let sprites_in_group: Vec<_> = character
        .sprite_groups
        .iter()
        .find(|g| g.name == current_group_name)
        .map(|g| g.sprites.clone())
        .unwrap_or_default();

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "サムネイルを差替え" }

                form { class: "space-y-3", onsubmit: on_submit,
                    if let Some(message) = error() {
                        div { role: "alert", class: "alert alert-error",
                            span { "{message}" }
                        }
                    }

                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "Sprite Group" }
                        select {
                            class: "select select-bordered w-full",
                            onchange: move |e| {
                                let new_group = e.value();
                                // group が変わったら sprite index を先頭に戻す
                                let first_idx = character
                                    .sprite_groups
                                    .iter()
                                    .find(|g| g.name == new_group)
                                    .and_then(|g| g.sprites.first())
                                    .map_or(0, |s| s.index);
                                selected_group.set(new_group);
                                selected_sprite_index.set(first_idx);
                            },
                            for group in character.sprite_groups.iter() {
                                option {
                                    key: "{group.name}",
                                    value: "{group.name}",
                                    selected: group.name == current_group_name,
                                    "{group.name}"
                                }
                            }
                        }
                    }

                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "Sprite" }
                        if sprites_in_group.is_empty() {
                            p { class: "text-base-content/60 italic text-sm",
                                "この Sprite Group に Sprite がありません"
                            }
                        } else {
                            select {
                                class: "select select-bordered w-full",
                                onchange: move |e| {
                                    if let Ok(v) = e.value().parse::<u32>() {
                                        selected_sprite_index.set(v);
                                    }
                                },
                                for sprite in sprites_in_group.iter() {
                                    option {
                                        key: "{sprite.index}",
                                        value: "{sprite.index}",
                                        selected: sprite.index == selected_sprite_index(),
                                        "#{sprite.index} {sprite.path}"
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
                        button {
                            r#type: "submit",
                            class: "btn btn-primary",
                            disabled: sprites_in_group.is_empty(),
                            "適用"
                        }
                    }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}
