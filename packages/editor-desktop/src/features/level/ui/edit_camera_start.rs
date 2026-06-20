use dioxus::prelude::*;

use crate::entities::level::Level;
use crate::shared::UseHistory;

/// Camera 開始位置 (X, Y) の inline 編集コンポーネント。
/// Pattern D: 親 (LevelEditor) の `draft` Signal を直接編集する。disk への保存は Save ボタン担当。
#[component]
pub fn EditCameraStart(mut draft: Signal<Level>, mut history: UseHistory<Level>) -> Element {
    let mut editing = use_signal(|| false);
    let initial = draft.peek().clone();
    let mut draft_x = use_signal(|| initial.camera_start_x.to_string());
    let mut draft_y = use_signal(|| initial.camera_start_y.to_string());
    let mut error = use_signal(|| None::<String>);

    let on_edit = move |_| {
        let cur = draft.peek().clone();
        draft_x.set(cur.camera_start_x.to_string());
        draft_y.set(cur.camera_start_y.to_string());
        error.set(None);
        editing.set(true);
    };

    let on_apply = move |_| {
        let Ok(new_x) = draft_x().trim().parse::<i32>() else {
            error.set(Some("X は整数で入力してください".into()));
            return;
        };
        let Ok(new_y) = draft_y().trim().parse::<i32>() else {
            error.set(Some("Y は整数で入力してください".into()));
            return;
        };
        let cur = draft.peek().clone();
        if cur.camera_start_x != new_x || cur.camera_start_y != new_y {
            history.record();
            draft.set(Level {
                camera_start_x: new_x,
                camera_start_y: new_y,
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

    let cur = draft.read();
    let display_x = cur.camera_start_x;
    let display_y = cur.camera_start_y;

    rsx! {
        if editing() {
            div { class: "flex flex-col gap-2",
                div { class: "flex items-center gap-1",
                    label { class: "text-xs text-base-content/60 w-4", "X" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        value: "{draft_x}",
                        step: "1",
                        oninput: move |e| draft_x.set(e.value()),
                    }
                }
                div { class: "flex items-center gap-1",
                    label { class: "text-xs text-base-content/60 w-4", "Y" }
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
                span { class: "font-mono text-sm", "({display_x}, {display_y})" }
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
