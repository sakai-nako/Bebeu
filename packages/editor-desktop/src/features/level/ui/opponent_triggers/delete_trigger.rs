use dioxus::prelude::*;

use crate::entities::level::Level;
use crate::shared::UseHistory;

/// Opponent trigger を Confirm Modal 付きで削除するボタン。
/// Pattern D: 親 (LevelEditor) の `draft` Signal を直接編集する。
#[component]
pub fn DeleteTriggerButton(
    mut draft: Signal<Level>,
    mut history: UseHistory<Level>,
    index: usize,
) -> Element {
    let mut show_modal = use_signal(|| false);

    let on_confirm = move |_| {
        let cur = draft.peek().clone();
        if index >= cur.opponent_triggers.len() {
            show_modal.set(false);
            return;
        }
        let mut new_triggers = cur.opponent_triggers.clone();
        new_triggers.remove(index);
        history.record();
        draft.set(Level {
            opponent_triggers: new_triggers,
            ..cur
        });
        show_modal.set(false);
    };

    let display_name = {
        let cur = draft.read();
        cur.opponent_triggers
            .get(index)
            .map(|t| {
                if t.character_name.is_empty() {
                    "(未設定)".to_string()
                } else {
                    t.character_name.clone()
                }
            })
            .unwrap_or_default()
    };

    rsx! {
        button {
            r#type: "button",
            class: "btn btn-error btn-outline btn-xs",
            onclick: move |_| show_modal.set(true),
            "削除"
        }

        if show_modal() {
            dialog { class: "modal modal-open",
                div { class: "modal-box",
                    h3 { class: "text-lg font-bold mb-2", "Trigger を削除" }
                    p { class: "py-2",
                        "Trigger #"
                        span { class: "font-mono font-semibold", "{index}" }
                        " ('"
                        span { class: "font-mono", "{display_name}" }
                        "') を削除しますか？"
                    }
                    div { class: "modal-action",
                        button {
                            r#type: "button",
                            class: "btn btn-ghost",
                            onclick: move |_| show_modal.set(false),
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
                    button {
                        r#type: "button",
                        onclick: move |_| show_modal.set(false),
                        "close"
                    }
                }
            }
        }
    }
}
