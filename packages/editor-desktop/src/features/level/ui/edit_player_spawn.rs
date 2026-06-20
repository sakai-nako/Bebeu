use dioxus::prelude::*;

use crate::entities::level::Level;
use crate::shared::UseHistory;

/// Player の初期 spawn 位置 (X, Z) の inline 編集コンポーネント。
/// 初期 spawn Y は常に 0 のため UI から省く (`player_respawn_y` は別コンポーネント)。
/// Pattern D: 親 (LevelEditor) の `draft` Signal を直接編集する。
#[component]
pub fn EditPlayerSpawn(mut draft: Signal<Level>, mut history: UseHistory<Level>) -> Element {
    let mut editing = use_signal(|| false);
    let initial = draft.peek().clone();
    let mut draft_x = use_signal(|| initial.player_spawn_x.to_string());
    let mut draft_z = use_signal(|| initial.player_spawn_z.to_string());
    let mut error = use_signal(|| None::<String>);

    let on_edit = move |_| {
        let cur = draft.peek().clone();
        draft_x.set(cur.player_spawn_x.to_string());
        draft_z.set(cur.player_spawn_z.to_string());
        error.set(None);
        editing.set(true);
    };

    let on_apply = move |_| {
        let Ok(new_x) = draft_x().trim().parse::<i32>() else {
            error.set(Some("X は整数で入力してください".into()));
            return;
        };
        let Ok(new_z) = draft_z().trim().parse::<i32>() else {
            error.set(Some("Z は整数で入力してください".into()));
            return;
        };
        let cur = draft.peek().clone();
        if cur.player_spawn_x != new_x || cur.player_spawn_z != new_z {
            history.record();
            draft.set(Level {
                player_spawn_x: new_x,
                player_spawn_z: new_z,
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
    let display_x = cur.player_spawn_x;
    let display_z = cur.player_spawn_z;

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
                    label { class: "text-xs text-base-content/60 w-4", "Z" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        value: "{draft_z}",
                        step: "1",
                        oninput: move |e| draft_z.set(e.value()),
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
                span { class: "font-mono text-sm", "X={display_x}, Y=0, Z={display_z}" }
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
