use dioxus::prelude::*;

use crate::entities::level::Level;
use crate::shared::UseHistory;

/// Player Respawn Y (死亡後の落下開始 Y) の inline 編集コンポーネント。
/// 0 で「ground line で即復活」、正の値で「上空から落下」を表す。
/// Pattern D: 親 (LevelEditor) の `draft` Signal を直接編集する。
#[component]
pub fn EditPlayerRespawnY(mut draft: Signal<Level>, mut history: UseHistory<Level>) -> Element {
    let mut editing = use_signal(|| false);
    let initial = draft.peek().clone();
    let mut draft_y = use_signal(|| initial.player_respawn_y.to_string());
    let mut error = use_signal(|| None::<String>);

    let on_edit = move |_| {
        let cur = draft.peek().clone();
        draft_y.set(cur.player_respawn_y.to_string());
        error.set(None);
        editing.set(true);
    };

    let on_apply = move |_| {
        let Ok(new_y) = draft_y().trim().parse::<i32>() else {
            error.set(Some("Y は整数で入力してください".into()));
            return;
        };
        let cur = draft.peek().clone();
        if cur.player_respawn_y != new_y {
            history.record();
            draft.set(Level {
                player_respawn_y: new_y,
                ..cur
            });
        }
        editing.set(false);
        error.set(None);
    };

    let on_cancel = move |_| {
        editing.set(false);
        error.set(None);
    };

    let display_y = draft.read().player_respawn_y;

    rsx! {
        if editing() {
            div { class: "flex flex-col gap-2",
                div { class: "flex items-center gap-1",
                    label { class: "text-xs text-base-content/60", "Respawn Y" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        value: "{draft_y}",
                        step: "1",
                        oninput: move |e| draft_y.set(e.value()),
                    }
                }
                div { class: "flex gap-1",
                    button {
                        r#type: "button",
                        class: "btn btn-primary btn-xs",
                        onclick: on_apply,
                        "Apply"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-ghost btn-xs",
                        onclick: on_cancel,
                        "Cancel"
                    }
                }
                if let Some(message) = error() {
                    p { class: "text-error text-xs", "{message}" }
                }
            }
        } else {
            div { class: "flex items-center gap-2",
                span { class: "font-mono text-sm", "Respawn Y = {display_y}" }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: on_edit,
                    title: "編集",
                    "✎"
                }
            }
        }
    }
}
