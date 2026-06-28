//! Frame に紐づく Sound 設定を編集する (ADR-0019 / ADR-0034)。
//!
//! 3 系統の SoundGroup 参照 (Default / On hit / On guard) と共通 Delay (ms) を持つ。
//!
//! 仕様:
//! - 各セレクタの先頭は "(none)" で `None` に対応する
//! - 続いて `character.sound_groups` を `number` 昇順で並べ、`{number} — {name}` で表示する
//! - 3 系統すべてが None になったら `Frame.sound = None` にする (= 完全に無発火、yaml にも
//!   sound キーが出ない)。1 つでも Some なら `Frame.sound = Some(FrameSound { ... })`
//! - Delay は 3 系統共通。少なくとも 1 つが Some のときだけ入力欄を出す

use dioxus::prelude::*;

use crate::entities::character::{Animation, Character, FrameSound};
use crate::shared::UseHistory;

/// どの系統 (Default / OnHit / OnGuard) のセレクタかを識別する。`on_change` 内で
/// 各分岐に書き分ける。
#[derive(Clone, Copy, PartialEq, Eq)]
enum SoundSlot {
    /// 既定 (= 振り音 / Hit voice / 通常時セリフ 等、`AttackOutcome::Idle` 時の選択先 +
    /// on_hit / on_guard が None のときのフォールバック)。用途は attack の swing に限らず
    /// 「無条件で frame 進入時に latch したい音」全般。
    Default,
    OnHit,
    OnGuard,
}

impl SoundSlot {
    fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::OnHit => "On hit",
            Self::OnGuard => "On guard",
        }
    }

    fn current(self, fs: Option<&FrameSound>) -> Option<u32> {
        fs.and_then(|s| match self {
            Self::Default => s.number,
            Self::OnHit => s.on_hit,
            Self::OnGuard => s.on_guard,
        })
    }
}

/// 3 系統の Option<u32> から `FrameSound` を作る。すべて None なら sound = None で返す
/// (= yaml に sound キー自体を出さない)。
fn build_sound(
    number: Option<u32>,
    on_hit: Option<u32>,
    on_guard: Option<u32>,
    delay_ms: u32,
) -> Option<FrameSound> {
    if number.is_none() && on_hit.is_none() && on_guard.is_none() {
        return None;
    }
    Some(FrameSound {
        number,
        on_hit,
        on_guard,
        delay_ms,
    })
}

#[component]
pub(super) fn FrameSoundSection(
    character: Character,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    selected: Option<FrameSound>,
) -> Element {
    let current_number = SoundSlot::Default.current(selected.as_ref());
    let current_on_hit = SoundSlot::OnHit.current(selected.as_ref());
    let current_on_guard = SoundSlot::OnGuard.current(selected.as_ref());
    let current_delay: u32 = selected.as_ref().map_or(0, |s| s.delay_ms);
    let any_set =
        current_number.is_some() || current_on_hit.is_some() || current_on_guard.is_some();

    // SoundGroup セレクタ変更ハンドラを生成する。slot の field だけ差し替え、他は据え置き。
    let make_on_change = move |slot: SoundSlot| {
        move |evt: Event<FormData>| {
            let value = evt.value();
            let new_value: Option<u32> = if value == "none" {
                None
            } else {
                value.parse::<u32>().ok()
            };
            let mut number = current_number;
            let mut on_hit = current_on_hit;
            let mut on_guard = current_on_guard;
            match slot {
                SoundSlot::Default => number = new_value,
                SoundSlot::OnHit => on_hit = new_value,
                SoundSlot::OnGuard => on_guard = new_value,
            }
            let new_sound = build_sound(number, on_hit, on_guard, current_delay);
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

    // Delay 変更ハンドラ。number は据え置き。すべて None のときは入力欄が出ないので呼ばれない。
    let on_delay = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        if current_delay == v {
            return;
        }
        let new_sound = build_sound(current_number, current_on_hit, current_on_guard, v);
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        f.sound = new_sound;
        history.record();
        draft.set(updated);
    };

    rsx! {
        div { class: "space-y-2",
            h3 { class: "font-semibold text-sm", "Sound" }
            for slot in [SoundSlot::Default, SoundSlot::OnHit, SoundSlot::OnGuard] {
                FrameSoundSlotSelector {
                    key: "{slot.label()}",
                    label: slot.label(),
                    current: slot.current(selected.as_ref()),
                    sound_groups: character.sound_groups.clone(),
                    on_change: make_on_change(slot),
                }
            }
            if any_set {
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

/// 1 系統ぶんの SoundGroup ドロップダウン。label + 現在値 + 全 SoundGroup option を描画する。
///
/// Dioxus desktop の webview では `<select value="...">` だけだと controlled state が DOM と
/// 同期せず、Animation を開いた直後の初回描画で現在値が反映されない (= "(none)" に見える) 症状が
/// 出る。各 `<option>` に `selected:` を併用することで強制的に同期させる
/// (`layers.rs` の sprite_group_number セレクタでも同じ対応を取っている)。
#[component]
fn FrameSoundSlotSelector(
    label: &'static str,
    current: Option<u32>,
    sound_groups: Vec<crate::entities::character::SoundGroup>,
    on_change: EventHandler<Event<FormData>>,
) -> Element {
    let current_value: String = current.map_or_else(|| "none".to_string(), |n| n.to_string());
    rsx! {
        div { class: "grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 items-center",
            label { class: "text-xs", "{label}" }
            select {
                class: "select select-bordered select-sm w-full",
                value: "{current_value}",
                onchange: move |evt| on_change.call(evt),
                option {
                    value: "none",
                    selected: current.is_none(),
                    "(none)"
                }
                for g in sound_groups.iter() {
                    option {
                        key: "{g.number}",
                        value: "{g.number}",
                        selected: current == Some(g.number),
                        "{g.number} — {g.name}"
                    }
                }
            }
        }
    }
}
