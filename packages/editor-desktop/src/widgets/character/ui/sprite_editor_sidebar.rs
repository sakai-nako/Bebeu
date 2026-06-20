use dioxus::prelude::*;

use super::SpriteThumbnail;
use super::canvas_common::is_primary_click;
use crate::entities::character::{Character, SelectedBox, SpriteGroup};
use crate::features::character::{ImportSpritesButton, ReimportSpritesScaledButton};
use crate::shared::{SpriteDiskOps, UseHistory};

#[component]
pub fn SpriteEditorSidebar(
    character: Character,
    sprite_group: SpriteGroup,
    mut draft: Signal<SpriteGroup>,
    mut history: UseHistory<SpriteGroup>,
    mut selected_sprite_index: Signal<usize>,
    mut selected_box: Signal<Option<SelectedBox>>,
    disk_ops: Signal<SpriteDiskOps>,
) -> Element {
    let group = draft();
    let group_name = group.name.clone();
    let current = selected_sprite_index();
    let character_name = character.name.clone();
    let sprites_count = group.sprites.len();

    // 並び替えドラッグの状態。HTML5 D&D は WebView2 で禁止カーソルが消せない問題があるので、
    // canvas の drag と同じく mousedown / mousemove / mouseup ベースで実装する。
    // dragging_index = ドラッグ中の行 index、drop_indicator = 挿入位置 (0..=len、len は末尾)。
    let mut dragging_index: Signal<Option<usize>> = use_signal(|| None);
    let mut drop_indicator: Signal<Option<usize>> = use_signal(|| None);

    let mut move_up = move |i: usize| {
        if i == 0 {
            return;
        }
        history.record();
        let mut updated = draft();
        if i >= updated.sprites.len() {
            return;
        }
        updated.sprites.swap(i - 1, i);
        draft.set(updated);
        // 選択追従: 選択中の sprite を移動した場合は新位置を選び直す
        if current == i {
            selected_sprite_index.set(i - 1);
        } else if current == i - 1 {
            selected_sprite_index.set(i);
        }
    };

    let mut move_down = move |i: usize| {
        let len = draft.peek().sprites.len();
        if i + 1 >= len {
            return;
        }
        history.record();
        let mut updated = draft();
        updated.sprites.swap(i, i + 1);
        draft.set(updated);
        if current == i {
            selected_sprite_index.set(i + 1);
        } else if current == i + 1 {
            selected_sprite_index.set(i);
        }
    };

    // reorder 本体。from を取り出して insert_before に挿入する。同位置 (= no-op) はスキップ。
    let mut perform_reorder = move |from: usize, insert_before: usize| {
        let len = draft.peek().sprites.len();
        if from >= len || insert_before > len {
            return;
        }
        if from == insert_before || from + 1 == insert_before {
            return;
        }
        history.record();
        let mut updated = draft();
        let sprite = updated.sprites.remove(from);
        // remove で from 以降が前詰めされたぶん、insert 位置を補正する
        let to = if insert_before > from {
            insert_before - 1
        } else {
            insert_before
        };
        updated.sprites.insert(to, sprite);
        draft.set(updated);

        // 選択追従: from が選択中なら追従、それ以外は remove + insert で index がズレる場合のみ補正
        let new_c = if current == from {
            to
        } else {
            let c_after_remove = if current < from { current } else { current - 1 };
            if to <= c_after_remove {
                c_after_remove + 1
            } else {
                c_after_remove
            }
        };
        if new_c != current {
            selected_sprite_index.set(new_c);
        }
    };

    let mut commit_or_cancel = move || {
        let from = *dragging_index.peek();
        let to = *drop_indicator.peek();
        dragging_index.set(None);
        drop_indicator.set(None);
        if let (Some(from), Some(to)) = (from, to) {
            perform_reorder(from, to);
        }
    };

    rsx! {
        div {
            class: "h-full overflow-y-auto p-2 space-y-2 bg-base-200 rounded-box",
            // ドラッグ中はリスト全体で mouseup を受けて drop を確定する。
            // mouseleave は「リスト外でリリースした」を検知して安全にキャンセルするためのフェイルセーフ。
            onmouseup: move |_| {
                if dragging_index.peek().is_some() {
                    commit_or_cancel();
                }
            },
            onmouseleave: move |_| {
                if dragging_index.peek().is_some() {
                    // リストから出たら drop 位置を見失う。canvas drag と同様にキャンセル扱いにする。
                    dragging_index.set(None);
                    drop_indicator.set(None);
                }
            },
            div { class: "px-1 space-y-1",
                h3 { class: "text-sm font-semibold", "Sprites ({sprites_count})" }
                div { class: "flex items-start gap-1 flex-wrap",
                    ImportSpritesButton {
                        character_name: character.name.clone(),
                        sprite_group_name: sprite_group.name.clone(),
                        draft,
                        history,
                        selected_sprite_index,
                        disk_ops,
                    }
                    ReimportSpritesScaledButton {
                        character_name: character.name.clone(),
                        sprite_group_name: sprite_group.name.clone(),
                        draft,
                        history,
                        selected_sprite_index,
                        disk_ops,
                    }
                }
            }
            if group.sprites.is_empty() {
                p { class: "text-xs text-base-content/60 italic px-1", "Sprite がありません。" }
            }
            for (i, sprite) in group.sprites.iter().enumerate() {
                // for body の key はトップレベル要素に必要。indicator は条件付き描画なので
                // 行と一体化させる wrapper を立てて、そこに key を載せる。
                div { key: "{sprite.index}",
                    // 行の上に drop indicator (insert-before セマンティクス)。
                    // ドラッグ元の上には出さない (no-op になる位置なので)。
                    if drop_indicator() == Some(i) {
                        div { class: "h-0.5 bg-warning rounded mx-1" }
                    }
                    div {
                        class: {
                            let base = if i == current {
                                "p-1 rounded outline outline-2 outline-primary cursor-pointer"
                            } else {
                                "p-1 rounded hover:bg-base-100 cursor-pointer"
                            };
                            let dragging = if dragging_index() == Some(i) { " opacity-40" } else { "" };
                            format!("{base}{dragging}")
                        },
                        onclick: move |_| {
                            selected_sprite_index.set(i);
                            selected_box.set(None);
                        },
                        // ドラッグ中に行に入ったら drop 位置を更新する。
                        // 自分自身に入った時は indicator を消す (no-op 位置なので)。
                        onmouseenter: move |_| {
                            if dragging_index.peek().is_none() {
                                return;
                            }
                            if *dragging_index.peek() == Some(i) {
                                if drop_indicator.peek().is_some() {
                                    drop_indicator.set(None);
                                }
                            } else if *drop_indicator.peek() != Some(i) {
                                drop_indicator.set(Some(i));
                            }
                        },
                        div { class: "flex items-stretch gap-1",
                            // 左カラム: ドラッグハンドル (mousedown で drag 開始)
                            div {
                                class: "w-5 shrink-0 flex items-center justify-center text-base-content/40 select-none cursor-grab active:cursor-grabbing",
                                title: "ドラッグして並び替え",
                                onmousedown: move |evt| {
                                    if !is_primary_click(&evt) {
                                        return;
                                    }
                                    evt.stop_propagation();
                                    dragging_index.set(Some(i));
                                    // drop_indicator は最初 None。別の行/末尾 zone に入った時点で立つ。
                                    drop_indicator.set(None);
                                },
                                "⋮⋮"
                            }
                            // 中央カラム: サムネイル（伸縮）
                            div { class: "flex-1 min-w-0",
                                SpriteThumbnail {
                                    character_name: character_name.clone(),
                                    sprite_group_name: group_name.clone(),
                                    index: sprite.index,
                                    path: sprite.path.clone(),
                                }
                            }
                            // 右カラム: 上下移動ボタン（縦並び・サムネイル高に対し縦中央寄せ）
                            div { class: "flex flex-col justify-center gap-1 shrink-0 w-7",
                                button {
                                    class: "btn btn-ghost btn-sm min-h-0 h-7 px-0",
                                    disabled: i == 0,
                                    title: "上に移動",
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        move_up(i);
                                    },
                                    "↑"
                                }
                                button {
                                    class: "btn btn-ghost btn-sm min-h-0 h-7 px-0",
                                    disabled: i + 1 >= sprites_count,
                                    title: "下に移動",
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        move_down(i);
                                    },
                                    "↓"
                                }
                            }
                        }
                    }
                }
            }
            // 末尾追加用の drop zone。最後の行の後ろにマウスが入ったら drop_indicator = sprites_count。
            // 高さ h-3 で hit area を確保しつつ、indicator が立った時だけ可視化する。
            if !group.sprites.is_empty() {
                div {
                    class: "h-3 mx-1 flex items-center",
                    onmouseenter: move |_| {
                        if dragging_index.peek().is_none() {
                            return;
                        }
                        if *drop_indicator.peek() != Some(sprites_count) {
                            drop_indicator.set(Some(sprites_count));
                        }
                    },
                    if drop_indicator() == Some(sprites_count) {
                        div { class: "h-0.5 w-full bg-warning rounded" }
                    }
                }
            }
        }
    }
}
