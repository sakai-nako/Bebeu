use dioxus::prelude::*;

use crate::entities::character::SpriteGroup;
use crate::shared::UseHistory;

/// 一つ前の Sprite (selected_index - 1) の Pivot / Body Box / Attack Box を
/// 現在選択中の Sprite に反映するボタン。連続フレームで前フレームの設定を
/// 引き継いで微調整するワークフロー向け。
/// 永続化は SpriteGroupEditorActions の Save に委ねるため、draft Signal を上書きするだけで完結する。
#[component]
pub fn ApplyPreviousSpriteButton(
    mut draft: Signal<SpriteGroup>,
    history: UseHistory<SpriteGroup>,
    selected_sprite_index: Signal<usize>,
) -> Element {
    let mut show_modal = use_signal(|| false);
    let current = selected_sprite_index();
    let total = draft.read().sprites.len();
    let disabled = current == 0 || total <= 1;

    let button_title = if total <= 1 {
        "他の Sprite がありません"
    } else if current == 0 {
        "前の Sprite がありません (Sprite[0] が選択中)"
    } else {
        "前の Sprite の Pivot / Body / Attack を選択中の Sprite に反映"
    };

    rsx! {
        button {
            class: "btn btn-warning btn-outline btn-sm",
            disabled,
            title: button_title,
            onclick: move |_| show_modal.set(true),
            "前から反映"
        }

        if show_modal() {
            ApplyPreviousSpriteModal {
                draft,
                history,
                selected_sprite_index,
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

#[component]
fn ApplyPreviousSpriteModal(
    mut draft: Signal<SpriteGroup>,
    mut history: UseHistory<SpriteGroup>,
    selected_sprite_index: Signal<usize>,
    onclose: EventHandler<()>,
) -> Element {
    let mut apply_pivot = use_signal(|| true);
    let mut apply_body = use_signal(|| true);
    let mut apply_attack = use_signal(|| true);

    let current = selected_sprite_index();
    let prev_index = current.saturating_sub(1);

    let on_confirm = move |_| {
        let mut updated = draft();
        // current が 0 の場合は disabled で button が押せない想定だが、
        // モーダル表示中に外で index が動く可能性に備えて防衛的に弾く。
        if current == 0 {
            onclose.call(());
            return;
        }
        // sprites[current-1] のクローンで「前 Sprite の値」を確保してから sprites[current] に書き戻す。
        // 借用ルール上、get_mut(current) で可変参照する間に get(current-1) を不変参照できないため。
        let Some(prev) = updated.sprites.get(current - 1).cloned() else {
            onclose.call(());
            return;
        };
        let pv = apply_pivot();
        let bb = apply_body();
        let ab = apply_attack();
        if let Some(s) = updated.sprites.get_mut(current) {
            if pv {
                s.pivot_point = prev.pivot_point;
            }
            if bb {
                s.body_boxes.clone_from(&prev.body_boxes);
            }
            if ab {
                s.attack_boxes.clone_from(&prev.attack_boxes);
            }
        }
        history.record();
        draft.set(updated);
        onclose.call(());
    };

    let nothing_selected = !apply_pivot() && !apply_body() && !apply_attack();

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-2", "前 Sprite から反映" }
                p { class: "py-2 text-sm",
                    "Sprite["
                    span { class: "font-semibold", "{prev_index}" }
                    "] の選択した項目を Sprite["
                    span { class: "font-semibold", "{current}" }
                    "] に上書き反映します。"
                }

                div { class: "flex flex-col gap-1 py-2",
                    label { class: "label cursor-pointer justify-start gap-2",
                        input {
                            r#type: "checkbox",
                            class: "checkbox checkbox-sm",
                            checked: apply_pivot(),
                            oninput: move |e| apply_pivot.set(e.checked()),
                        }
                        span { class: "label-text", "Pivot Point" }
                    }
                    label { class: "label cursor-pointer justify-start gap-2",
                        input {
                            r#type: "checkbox",
                            class: "checkbox checkbox-sm",
                            checked: apply_body(),
                            oninput: move |e| apply_body.set(e.checked()),
                        }
                        span { class: "label-text", "Body Box" }
                    }
                    label { class: "label cursor-pointer justify-start gap-2",
                        input {
                            r#type: "checkbox",
                            class: "checkbox checkbox-sm",
                            checked: apply_attack(),
                            oninput: move |e| apply_attack.set(e.checked()),
                        }
                        span { class: "label-text", "Attack Box" }
                    }
                }

                div { role: "alert", class: "alert alert-warning mt-2",
                    span { "選択した項目は現在の Sprite で完全に上書きされます。" }
                }

                div { class: "modal-action",
                    button {
                        class: "btn btn-ghost",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-warning",
                        disabled: nothing_selected,
                        onclick: on_confirm,
                        "反映"
                    }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}
