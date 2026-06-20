use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dioxus::prelude::*;

use super::FrameThumbnail;
use crate::entities::character::{
    Animation, Character, Frame, PlaybackConfig, PlaybackState, new_cancel_token,
    spawn_playback_thread, use_playback,
};
use crate::entities::keybinding::Action;
use crate::features::keybinding::use_keyboard_action;
use crate::shared::UseHistory;

/// 新規 Frame のデフォルト duration (ms)
const DEFAULT_FRAME_DURATION: u32 = 50;

/// frames の `index` を配列順に揃える。add / delete / move / duplicate のあとに必ず呼ぶ。
fn renumber_frames(animation: &mut Animation) {
    for (i, f) in animation.frames.iter_mut().enumerate() {
        f.index = u32::try_from(i).unwrap_or(u32::MAX);
    }
}

#[component]
pub fn AnimationTimeline(
    character: Character,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    mut selected_frame_index: Signal<usize>,
    mut selected_layer_index: Signal<Option<usize>>,
) -> Element {
    let frames = draft().frames.clone();
    let current = selected_frame_index();
    let frames_len = frames.len();

    // 再生状態とタイマースレッドの cancellation token (Timeline 内に閉じる)
    let mut playback = use_playback();
    let mut cancel_token: Signal<Option<Arc<AtomicBool>>> = use_signal(|| None);
    let state = playback();
    let locked = state.locks_editing();

    // 再生スレッドが書き込む pending frame index (Sync bridge)。UI 側で消費して selected_frame_index に流す。
    let mut pending_frame: Signal<Option<usize>, SyncStorage> = use_signal_sync(|| None);
    // 再生スレッドが参照する Animation 情報のミラー (Sync)。draft が変わるたびに use_effect で更新する。
    let mut config: Signal<PlaybackConfig, SyncStorage> =
        use_signal_sync(|| PlaybackConfig::from_animation(&draft.peek()));

    // draft 更新を Sync の config にミラーする
    use_effect(move || {
        let snap = draft();
        config.set(PlaybackConfig::from_animation(&snap));
    });

    // 再生スレッドが書いた pending frame を selected_frame_index に転送して None に戻す
    use_effect(move || {
        if let Some(idx) = pending_frame() {
            selected_frame_index.set(idx);
            pending_frame.set(None);
        }
    });

    // unmount 時にタイマースレッドが残らないように cancel フラグを立てる
    use_drop(move || {
        if let Some(token) = cancel_token.peek().as_ref() {
            token.store(true, Ordering::Relaxed);
        }
    });

    let on_select = use_callback(move |i: usize| {
        // 再生中はクリックでの seek を無効化 (Pause か Stop してから操作する)
        if playback.peek().locks_editing() {
            return;
        }
        selected_frame_index.set(i);
        selected_layer_index.set(None);
    });

    // Play / Pause / Stop の本体ロジック。onclick とキーボード shortcut から共用するために
    // use_callback で切り出す。引数 () で副作用のみ。
    let toggle_play_pause = use_callback(move |()| {
        // frames が空の場合は再生不能。Pause も意味がないので no-op。
        if config.peek().frame_durations.is_empty() {
            return;
        }
        // peek() の guard が match 中に生き続けると後続の playback.set() と借用衝突するので
        // 値を Copy で取り出してから match する。
        let current = *playback.peek();
        match current {
            PlaybackState::Playing => {
                // Pause: タイマー停止、frame_index 維持
                if let Some(token) = cancel_token.peek().as_ref() {
                    token.store(true, Ordering::Relaxed);
                }
                playback.set(PlaybackState::Paused);
            }
            PlaybackState::Stopped | PlaybackState::Paused => {
                // Play (Stopped or Paused): 既存スレッドを cancel して新規起動
                if let Some(old) = cancel_token.peek().as_ref() {
                    old.store(true, Ordering::Relaxed);
                }
                let token = new_cancel_token();
                cancel_token.set(Some(token.clone()));
                let initial = *selected_frame_index.peek();
                playback.set(PlaybackState::Playing);
                spawn_playback_thread(playback, pending_frame, config, initial, token);
            }
        }
    });

    let stop_playback = use_callback(move |()| {
        if let Some(token) = cancel_token.peek().as_ref() {
            token.store(true, Ordering::Relaxed);
        }
        playback.set(PlaybackState::Stopped);
        selected_frame_index.set(0);
    });

    // Keyboard shortcuts: Space で Play/Pause トグル、Shift+Space で Stop
    use_keyboard_action(Action::PlayPauseAnimation, move || {
        toggle_play_pause.call(());
    });
    use_keyboard_action(Action::StopAnimation, move || stop_playback.call(()));

    let on_play = move |_evt: MouseEvent| toggle_play_pause.call(());
    let on_pause = move |_evt: MouseEvent| toggle_play_pause.call(());
    let on_stop = move |_evt: MouseEvent| stop_playback.call(());

    let on_add = move |_| {
        history.record();
        let mut updated = draft();
        let new_frame = Frame {
            index: u32::try_from(updated.frames.len()).unwrap_or(u32::MAX),
            duration: DEFAULT_FRAME_DURATION,
            flip: None,
            pivot_point_offset: None,
            body_box_overrides: None,
            attack_box_overrides: None,
            sound: None,
            layers: Vec::new(),
        };
        let new_index = updated.frames.len();
        updated.frames.push(new_frame);
        renumber_frames(&mut updated);
        draft.set(updated);
        selected_frame_index.set(new_index);
        selected_layer_index.set(None);
    };

    let on_move_left = move |_| {
        let i = selected_frame_index();
        if i == 0 {
            return;
        }
        history.record();
        let mut updated = draft();
        if i >= updated.frames.len() {
            return;
        }
        updated.frames.swap(i - 1, i);
        renumber_frames(&mut updated);
        draft.set(updated);
        selected_frame_index.set(i - 1);
    };

    let on_move_right = move |_| {
        let i = selected_frame_index();
        let len = draft.peek().frames.len();
        if i + 1 >= len {
            return;
        }
        history.record();
        let mut updated = draft();
        updated.frames.swap(i, i + 1);
        renumber_frames(&mut updated);
        draft.set(updated);
        selected_frame_index.set(i + 1);
    };

    let on_duplicate = move |_| {
        let i = selected_frame_index();
        let len = draft.peek().frames.len();
        if i >= len {
            return;
        }
        history.record();
        let mut updated = draft();
        let mut copy = updated.frames[i].clone();
        copy.index = u32::try_from(i + 1).unwrap_or(u32::MAX);
        updated.frames.insert(i + 1, copy);
        renumber_frames(&mut updated);
        draft.set(updated);
        selected_frame_index.set(i + 1);
        selected_layer_index.set(None);
    };

    // 選択中フレームを複製しつつ、各 Layer の sprite_index を +1 した次フレームを追加
    let on_add_next_sprite = move |_| {
        let i = selected_frame_index();
        let len = draft.peek().frames.len();
        if i >= len {
            return;
        }
        history.record();
        let mut updated = draft();
        let mut copy = updated.frames[i].clone();
        copy.index = u32::try_from(i + 1).unwrap_or(u32::MAX);
        for layer in &mut copy.layers {
            layer.sprite_index = layer.sprite_index.saturating_add(1);
        }
        updated.frames.insert(i + 1, copy);
        renumber_frames(&mut updated);
        draft.set(updated);
        selected_frame_index.set(i + 1);
        selected_layer_index.set(None);
    };

    let on_delete = move |_| {
        let i = selected_frame_index();
        let len = draft.peek().frames.len();
        if i >= len {
            return;
        }
        history.record();
        let mut updated = draft();
        updated.frames.remove(i);
        renumber_frames(&mut updated);
        let new_len = updated.frames.len();
        draft.set(updated);
        // 選択 index を有効範囲にクランプ
        if new_len == 0 {
            selected_frame_index.set(0);
        } else {
            selected_frame_index.set(i.min(new_len - 1));
        }
        selected_layer_index.set(None);
    };

    let has_selected = current < frames_len;

    let is_playing = state == PlaybackState::Playing;
    let is_stopped = state == PlaybackState::Stopped;
    // Play は現在 Playing もしくは frames が空のとき押せない
    let play_disabled = is_playing || frames_len == 0;
    // Pause は Playing 中だけ押せる
    let pause_disabled = !is_playing;
    // Stop は何かしら再生コンテキスト (Playing or Paused) があるときだけ押せる
    let stop_disabled = is_stopped;
    // Pause 表示は Paused のときだけ active 風に
    let pause_class = if state == PlaybackState::Paused {
        "btn btn-xs btn-primary"
    } else {
        "btn btn-xs btn-outline"
    };

    rsx! {
        div { class: "flex flex-col gap-2 bg-base-200 rounded-box p-2",
            // ツールバー
            div { class: "flex items-center gap-2 flex-wrap",
                // 再生コントロール (Timeline 左端)
                button {
                    class: "btn btn-xs btn-primary",
                    disabled: play_disabled,
                    title: "再生",
                    onclick: on_play,
                    "▶"
                }
                button {
                    class: "{pause_class}",
                    disabled: pause_disabled,
                    title: "一時停止",
                    onclick: on_pause,
                    "⏸"
                }
                button {
                    class: "btn btn-xs btn-outline",
                    disabled: stop_disabled,
                    title: "停止 (先頭フレームに戻る)",
                    onclick: on_stop,
                    "⏹"
                }
                div { class: "divider divider-horizontal mx-0" }
                span { class: "text-xs font-semibold", "Frames ({frames_len})" }
                div { class: "divider divider-horizontal mx-0" }
                button {
                    class: "btn btn-xs",
                    disabled: !has_selected || current == 0 || locked,
                    title: "前のフレームと入れ替え",
                    onclick: on_move_left,
                    "← 前へ"
                }
                button {
                    class: "btn btn-xs",
                    disabled: !has_selected || current + 1 >= frames_len || locked,
                    title: "次のフレームと入れ替え",
                    onclick: on_move_right,
                    "次へ →"
                }
                button {
                    class: "btn btn-xs",
                    disabled: !has_selected || locked,
                    title: "選択フレームを複製",
                    onclick: on_duplicate,
                    "Duplicate"
                }
                button {
                    class: "btn btn-xs btn-error btn-outline",
                    disabled: !has_selected || locked,
                    title: "選択フレームを削除",
                    onclick: on_delete,
                    "Delete"
                }
                div { class: "ml-auto" }
                button {
                    class: "btn btn-primary btn-outline btn-xs",
                    disabled: !has_selected || locked,
                    title: "前フレームを複製し、各 Layer の Sprite Index を +1 した次フレームを追加",
                    onclick: on_add_next_sprite,
                    "+ Next Sprite"
                }
                button {
                    class: "btn btn-primary btn-xs",
                    disabled: locked,
                    onclick: on_add,
                    "+ Frame"
                }
            }

            // strip 本体: 横スクロール。outline-2 が overflow-x-auto の y 軸クリップに引っかかるので
            // 上下にも padding を取って outline が見切れないようにする。
            div { class: "flex flex-row gap-2 overflow-x-auto p-1",
                if frames.is_empty() {
                    div { class: "text-base-content/60 italic text-sm px-2 py-3",
                        "Frame がありません。「+ Frame」で追加してください。"
                    }
                }
                for (i, frame) in frames.iter().enumerate() {
                    div {
                        key: "{frame.index}",
                        class: if i == current { "shrink-0 rounded outline outline-2 outline-primary cursor-pointer" } else { "shrink-0 rounded hover:bg-base-100 cursor-pointer" },
                        onclick: move |_| on_select.call(i),
                        FrameThumbnail {
                            character: character.clone(),
                            frame: frame.clone(),
                        }
                    }
                }
            }
        }
    }
}
