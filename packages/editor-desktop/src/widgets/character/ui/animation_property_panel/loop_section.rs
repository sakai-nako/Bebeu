use dioxus::prelude::*;

use crate::entities::character::Animation;
use crate::shared::UseHistory;

#[component]
pub(super) fn AnimationLoopSection(
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
) -> Element {
    let (is_loop, loop_start, frame_count) = {
        let snap = draft.read();
        (
            snap.is_loop,
            snap.loop_start_index,
            u32::try_from(snap.frames.len()).unwrap_or(u32::MAX),
        )
    };
    let max_index = frame_count.saturating_sub(1);

    let on_toggle = move |evt: Event<FormData>| {
        let new_loop = evt.checked();
        let mut updated = draft();
        if updated.is_loop == new_loop {
            return;
        }
        history.record();
        updated.is_loop = new_loop;
        draft.set(updated);
    };

    let on_start = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        let clamped = v.min(max_index);
        let mut updated = draft();
        if updated.loop_start_index == clamped {
            return;
        }
        history.record();
        updated.loop_start_index = clamped;
        draft.set(updated);
    };

    rsx! {
        div { class: "space-y-2",
            h3 { class: "font-semibold", "Animation" }
            div { class: "grid grid-cols-[auto_1fr] gap-x-2 gap-y-2 items-center",
                label { class: "text-xs", "Loop" }
                input {
                    r#type: "checkbox",
                    class: "toggle toggle-sm",
                    checked: is_loop,
                    oninput: on_toggle,
                }
                if is_loop {
                    label { class: "text-xs", "Loop Start" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        min: "0",
                        max: "{max_index}",
                        value: "{loop_start}",
                        onchange: on_start,
                    }
                }
            }
        }
    }
}
