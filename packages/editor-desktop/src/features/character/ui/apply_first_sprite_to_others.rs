use dioxus::prelude::*;

use crate::entities::character::SpriteGroup;
use crate::shared::UseHistory;

/// Sprite[0] の Pivot / Body Box / Attack Box を index >= 1 の Sprite へ一括反映するボタン。
/// 永続化は SpriteGroupEditorActions の Save に委ねるため、draft Signal を上書きするだけで完結する。
#[component]
pub fn ApplyFirstSpriteButton(
    mut draft: Signal<SpriteGroup>,
    history: UseHistory<SpriteGroup>,
) -> Element {
    let mut show_modal = use_signal(|| false);
    let disabled = draft.read().sprites.len() <= 1;

    let button_title = if disabled {
        "他の Sprite がありません"
    } else {
        "Sprite[0] の Pivot / Body / Attack を他 Sprite に一括反映"
    };

    rsx! {
        button {
            class: "btn btn-warning btn-outline btn-sm",
            disabled,
            title: button_title,
            onclick: move |_| show_modal.set(true),
            "0番から反映"
        }

        if show_modal() {
            ApplyFirstSpriteModal {
                draft,
                history,
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

#[component]
fn ApplyFirstSpriteModal(
    mut draft: Signal<SpriteGroup>,
    mut history: UseHistory<SpriteGroup>,
    onclose: EventHandler<()>,
) -> Element {
    let mut apply_pivot = use_signal(|| true);
    let mut apply_body = use_signal(|| true);
    let mut apply_attack = use_signal(|| true);

    let target_count = draft.read().sprites.len().saturating_sub(1);

    let on_confirm = move |_| {
        let mut updated = draft();
        // sprites.first() のクローンで「0番固定の値」を確保してから他 Sprite に書き戻す。
        // 借用ルール上、iter_mut().skip(1) で他 Sprite を可変参照する間 first を不変参照できないため。
        let Some(first) = updated.sprites.first().cloned() else {
            onclose.call(());
            return;
        };
        let pv = apply_pivot();
        let bb = apply_body();
        let ab = apply_attack();
        for s in updated.sprites.iter_mut().skip(1) {
            if pv {
                s.pivot_point = first.pivot_point;
            }
            if bb {
                s.body_boxes.clone_from(&first.body_boxes);
            }
            if ab {
                s.attack_boxes.clone_from(&first.attack_boxes);
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
                h3 { class: "text-lg font-bold mb-2", "Sprite[0] から一括反映" }
                p { class: "py-2 text-sm",
                    "Sprite[0] の選択した項目を他 "
                    span { class: "font-semibold", "{target_count}" }
                    " 件の Sprite に上書き反映します。"
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
                    span { "選択した項目は他の Sprite で完全に上書きされます。" }
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
