use dioxus::prelude::*;

use crate::entities::character::Animation;
use crate::shared::UseHistory;

#[component]
pub(super) fn FramePropertiesSection(
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    duration: u32,
) -> Element {
    let on_duration = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        if f.duration == v {
            return;
        }
        f.duration = v;
        history.record();
        draft.set(updated);
    };

    rsx! {
        div { class: "space-y-2",
            h3 { class: "font-semibold", "Frame #{frame_index}" }
            div { class: "grid grid-cols-[auto_1fr] gap-x-2 gap-y-2 items-center",
                label { class: "text-xs", "Duration (ms)" }
                input {
                    r#type: "number",
                    class: "input input-bordered input-sm w-24",
                    min: "0",
                    value: "{duration}",
                    onchange: on_duration,
                }
            }
        }
    }
}
