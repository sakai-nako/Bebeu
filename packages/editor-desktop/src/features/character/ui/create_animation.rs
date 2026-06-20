use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{
    Animation, Character, CharacterRepository, Role, use_characters_refresh,
};

const DEFAULT_IS_LOOP: bool = true;
const DEFAULT_LOOP_START_INDEX: u32 = 0;

// Role <-> YAML 表現の変換は `Role::yaml_value` / `Role::from_yaml_value` に集約してある (role.rs)。
// 旧 `role: dead` の DeadLieDown 読み替えもそこで対応。

#[component]
pub fn CreateAnimationButton(character: Character) -> Element {
    let mut show_modal = use_signal(|| false);
    let modal_character = character.clone();

    rsx! {
        button {
            class: "btn btn-primary btn-sm",
            onclick: move |_| show_modal.set(true),
            "+ New"
        }

        if show_modal() {
            CreateAnimationModal {
                character: modal_character.clone(),
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

#[component]
fn CreateAnimationModal(character: Character, onclose: EventHandler<()>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let mut name = use_signal(String::new);
    let mut role_input = use_signal(|| Role::Custom);
    let mut variant_input = use_signal(|| 0_u32);
    let mut export_number_input = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let new_name = name();
        let new_name_trimmed = new_name.trim();
        if new_name_trimmed.is_empty() {
            error.set(Some("Name は必須です".into()));
            return;
        }
        if original
            .animations
            .iter()
            .any(|a| a.name == new_name_trimmed)
        {
            error.set(Some(format!(
                "Animation '{new_name_trimmed}' は既に存在します"
            )));
            return;
        }

        let new_role = role_input();
        let new_variant = if new_role.is_single_cardinality() {
            0
        } else {
            variant_input()
        };

        // Role + variant 重複チェック (single なら role のみ、multi なら (role, variant) ペア)。
        // Custom は role 衝突対象外。
        if new_role != Role::Custom {
            let conflict = original.animations.iter().find(|a| {
                a.role == new_role && (new_role.is_single_cardinality() || a.variant == new_variant)
            });
            if let Some(c) = conflict {
                let msg = if new_role.is_single_cardinality() {
                    format!(
                        "Role '{}' は既に '{}' に割り当てられています",
                        new_role.display_label(),
                        c.name
                    )
                } else {
                    format!(
                        "Role '{}' の variant {} は既に '{}' に割り当てられています",
                        new_role.display_label(),
                        new_variant,
                        c.name
                    )
                };
                error.set(Some(msg));
                return;
            }
        }

        // Custom 時のみ export_number を読む (それ以外は None)。
        let new_export_number = if new_role == Role::Custom {
            let trimmed = export_number_input();
            let trimmed = trimmed.trim();
            if trimmed.is_empty() {
                None
            } else if let Ok(n) = trimmed.parse::<u32>() {
                // 既存 Custom Animation の export_number との重複を弾く。
                if original
                    .animations
                    .iter()
                    .any(|a| a.role == Role::Custom && a.export_number == Some(n))
                {
                    error.set(Some(format!(
                        "Export Number {n} は既に他の Custom Animation で使われています"
                    )));
                    return;
                }
                Some(n)
            } else {
                error.set(Some(
                    "Export Number は 0 以上の整数で入力してください".into(),
                ));
                return;
            }
        } else {
            None
        };

        let mut updated = original.clone();
        updated.animations.push(Animation {
            name: new_name_trimmed.to_string(),
            role: new_role,
            variant: new_variant,
            export_number: new_export_number,
            is_loop: DEFAULT_IS_LOOP,
            loop_start_index: DEFAULT_LOOP_START_INDEX,
            frames: Vec::new(),
        });

        match repo.update(&updated) {
            Ok(()) => {
                refresh.bump();
                onclose.call(());
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "新規 Animation" }

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
                        legend { class: "fieldset-legend", "Role" }
                        select {
                            class: "select select-bordered w-full",
                            value: "{Role::yaml_value(role_input())}",
                            onchange: move |e| {
                                if let Some(r) = Role::from_yaml_value(&e.value()) {
                                    role_input.set(r);
                                    if r.is_single_cardinality() {
                                        variant_input.set(0);
                                    }
                                }
                            },
                            for r in Role::all().iter().copied() {
                                option {
                                    value: Role::yaml_value(r),
                                    selected: r == role_input(),
                                    "{r.selector_label()}"
                                }
                            }
                        }
                    }

                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "Variant" }
                        input {
                            r#type: "number",
                            class: "input input-bordered w-full",
                            min: "0",
                            value: "{variant_input}",
                            disabled: role_input().is_single_cardinality(),
                            onchange: move |e| {
                                if let Ok(v) = e.value().trim().parse::<u32>() {
                                    variant_input.set(v);
                                }
                            },
                        }
                    }

                    if role_input() == Role::Custom {
                        fieldset { class: "fieldset",
                            legend {
                                class: "fieldset-legend",
                                title: "ikemen export 時に独自 CNS state controller (ChangeAnim 等) から参照する独自 Action 番号。空のままなら ikemen に出力しない",
                                "Export Number (任意)"
                            }
                            input {
                                r#type: "number",
                                class: "input input-bordered w-full",
                                min: "0",
                                value: "{export_number_input}",
                                oninput: move |e| export_number_input.set(e.value()),
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
