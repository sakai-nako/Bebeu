//! Frame に紐づく Sound (SoundGroup.Number 参照 + 再生遅延) を編集する。
//!
//! 仕様:
//! - 選択肢の先頭は "(none)" で、Frame.sound = None に対応する
//! - 続いて character.sound_groups を `number` 昇順で並べ、`{number} — {name}` で表示する
//!   (number はキャラ作者が手で振る ID なので明示的に出す)
//! - 選択を変えた時だけ history.record() してから draft を更新する
//! - SoundGroup を選択中のみ Delay (ms) 入力を表示する。delay は number と独立に編集できる

use dioxus::prelude::*;

use crate::entities::character::{Animation, Character, FrameSound};
use crate::shared::UseHistory;

#[component]
pub(super) fn FrameSoundSection(
    character: Character,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    selected: Option<FrameSound>,
) -> Element {
    // SoundGroup 切り替え。delay_ms は据え置き（"sound だけ差し替え" の意図）。
    let on_change = {
        let selected = selected.clone();
        move |evt: Event<FormData>| {
            let value = evt.value();
            let new_sound: Option<FrameSound> = if value == "none" {
                None
            } else {
                value.parse::<u32>().ok().map(|number| FrameSound {
                    number,
                    delay_ms: selected.as_ref().map_or(0, |s| s.delay_ms),
                })
            };
            let mut updated = draft();
            let Some(f) = updated.frames.get_mut(frame_index) else {
                return;
            };
            if f.sound == new_sound {
                return;
            }
            f.sound = new_sound;
            history.record();
            draft.set(updated);
        }
    };

    // Delay 変更。number は据え置き。sound = None の時は呼ばれない (input が出ていないため)。
    let on_delay = {
        let selected = selected.clone();
        move |evt: Event<FormData>| {
            let Ok(v) = evt.value().trim().parse::<u32>() else {
                return;
            };
            let Some(current) = selected.clone() else {
                return;
            };
            if current.delay_ms == v {
                return;
            }
            let mut updated = draft();
            let Some(f) = updated.frames.get_mut(frame_index) else {
                return;
            };
            f.sound = Some(FrameSound {
                number: current.number,
                delay_ms: v,
            });
            history.record();
            draft.set(updated);
        }
    };

    let current_value: String = selected
        .as_ref()
        .map_or_else(|| "none".to_string(), |s| s.number.to_string());
    let current_delay: u32 = selected.as_ref().map_or(0, |s| s.delay_ms);
    let has_sound = selected.is_some();

    rsx! {
        div { class: "space-y-2",
            h3 { class: "font-semibold text-sm", "Sound" }
            select {
                class: "select select-bordered select-sm w-full",
                value: "{current_value}",
                onchange: on_change,
                option { value: "none", "(none)" }
                for g in character.sound_groups.iter() {
                    option { key: "{g.number}", value: "{g.number}", "{g.number} — {g.name}" }
                }
            }
            if has_sound {
                div { class: "grid grid-cols-[auto_1fr] gap-x-2 gap-y-2 items-center",
                    label { class: "text-xs", "Delay (ms)" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        min: "0",
                        value: "{current_delay}",
                        onchange: on_delay,
                    }
                }
            }
            if character.sound_groups.is_empty() {
                p { class: "text-xs text-base-content/60 italic",
                    "SoundGroup がありません。data/characters/{character.name}/sound-groups/ に yaml を追加してください。"
                }
            }
        }
    }
}
