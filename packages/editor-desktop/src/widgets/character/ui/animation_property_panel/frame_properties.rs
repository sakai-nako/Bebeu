use dioxus::prelude::*;

use crate::entities::character::Animation;
use crate::shared::UseHistory;

/// `ticks` を 60Hz 想定で ms 換算した文字列を作る (UI 補助表示用)。
/// 例: 7 → "≈ 116.7 ms"。
fn ticks_to_approx_ms(ticks: u32) -> String {
    let ms = f64::from(ticks) * (1000.0 / 60.0);
    format!("≈ {ms:.1} ms")
}

#[component]
pub(super) fn FramePropertiesSection(
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    ticks: u32,
) -> Element {
    let on_ticks = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        if f.ticks == v {
            return;
        }
        f.ticks = v;
        history.record();
        draft.set(updated);
    };

    let approx = ticks_to_approx_ms(ticks);

    rsx! {
        div { class: "space-y-2",
            h3 { class: "font-semibold", "Frame #{frame_index}" }
            div { class: "grid grid-cols-[auto_1fr] gap-x-2 gap-y-2 items-center",
                label { class: "text-xs", "Ticks (60Hz)" }
                div { class: "flex items-center gap-2",
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        min: "0",
                        value: "{ticks}",
                        onchange: on_ticks,
                    }
                    span { class: "text-xs text-base-content/60 font-mono", "{approx}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_to_approx_ms_zero() {
        assert_eq!(ticks_to_approx_ms(0), "≈ 0.0 ms");
    }

    #[test]
    fn ticks_to_approx_ms_seven_is_about_one_sixth_second() {
        // 7 / 60 = 0.11667 s = 116.667 ms → 1 桁丸めで "116.7 ms"
        assert_eq!(ticks_to_approx_ms(7), "≈ 116.7 ms");
    }

    #[test]
    fn ticks_to_approx_ms_sixty_is_one_second() {
        assert_eq!(ticks_to_approx_ms(60), "≈ 1000.0 ms");
    }
}
