use std::sync::Arc;

use dioxus::prelude::*;

use super::attack_meta_inputs::AttackMetaInputs;
use super::hitbox_inputs::{HitBoxCornerInput, HitBoxDepthInput};
use super::sprite_reference::{ReferenceSection, SpriteReference};
use crate::entities::character::{
    BoxKind, Character, CharacterRepository, SelectedBox, Sprite, SpriteGroup,
};
use crate::shared::{
    AttackBoxMeta, HitBox, HitBoxCorner, SpriteDiskOps, UseHistory, use_image_cache_buster,
};

#[component]
pub fn SpritePropertyPanel(
    character: Character,
    character_name: String,
    sprite_group_name: String,
    mut draft: Signal<SpriteGroup>,
    mut history: UseHistory<SpriteGroup>,
    selected_sprite_index: ReadSignal<usize>,
    selected_box: Signal<Option<SelectedBox>>,
    mut disk_ops: Signal<SpriteDiskOps>,
    references: Signal<Vec<SpriteReference>>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    // 差し替えで同 basename の disk 上書き / 過去キャッシュ衝突を回避するための bump 用。
    let cache_buster = use_image_cache_buster();
    let mut replace_error = use_signal(|| None::<String>);

    let sprite_index = selected_sprite_index();
    let sprite = {
        let read = draft.read();
        read.sprites.get(sprite_index).cloned()
    };
    let Some(sprite) = sprite else {
        return rsx! {
            div { class: "p-3 bg-base-200 rounded-box text-sm text-base-content/60 italic",
                "Sprite が選択されていません。"
            }
        };
    };

    let pivot = sprite.pivot_point;

    // 画像差し替え: rfd で別画像を選び、disk に新画像をコピーして draft.path を更新する。
    //
    // ## アトミック性
    //
    // - 新画像は `import_sprite_image` で disk にコピーし、basename を `disk_ops.pending_imports`
    //   に積む（Save で commit、Cancel で削除）
    // - 旧画像の扱いは「同セッション内で import した分か、commit 済みか」で分岐:
    //   - pending_imports にあった → そのセッションでの import を取り消す形で **即削除**
    //   - 無かった（= 前回 Save 済みの画像）→ `pending_deletions` に積み、Save 時に削除。
    //     Cancel すれば disk に残るので元に戻せる
    let on_replace = {
        let character_name = character_name.clone();
        let sprite_group_name = sprite_group_name.clone();
        let repo = repo.clone();
        move |_| {
            let Some(path) = rfd::FileDialog::new()
                .set_title("差し替える画像を選択")
                .add_filter("画像", &["png", "jpg", "jpeg", "gif", "webp", "svg"])
                .pick_file()
            else {
                return;
            };
            let Some(new_basename) = path.file_name().and_then(|n| n.to_str()).map(String::from)
            else {
                replace_error.set(Some(format!("無効なファイル名: {}", path.display())));
                return;
            };
            // 同 SpriteGroup 内の他 sprite と path 衝突チェック（差し替え対象自身は除外）
            let snapshot = draft.peek().clone();
            if snapshot
                .sprites
                .iter()
                .enumerate()
                .any(|(j, s)| j != sprite_index && s.path == new_basename)
            {
                replace_error.set(Some(format!(
                    "Sprite path '{new_basename}' は既に他の Sprite で使われています"
                )));
                return;
            }
            let old_path = match snapshot.sprites.get(sprite_index) {
                Some(s) => s.path.clone(),
                None => return,
            };

            match repo.import_sprite_image(&character_name, &sprite_group_name, &path) {
                Ok(basename) => {
                    // 旧画像の処理
                    let mut ops = disk_ops();
                    let still_used_in_draft = snapshot
                        .sprites
                        .iter()
                        .enumerate()
                        .any(|(j, s)| j != sprite_index && s.path == old_path);
                    if !still_used_in_draft && old_path != basename {
                        if ops.take_pending_import(&old_path) {
                            // 同セッション内 import の取り消し: 即 disk から削除
                            let _ = repo.delete_sprite_image(
                                &character_name,
                                &sprite_group_name,
                                &old_path,
                            );
                        } else {
                            // commit 済みの画像: Save 時に削除（Cancel なら残る）
                            ops.add_pending_deletion(old_path);
                        }
                    }
                    // 新画像を pending_imports に積む（同 basename を上書きしたなら一旦 take してから push し直し）
                    ops.take_pending_import(&basename);
                    ops.add_pending_import(basename.clone());
                    disk_ops.set(ops);

                    history.record();
                    let mut updated = draft();
                    if let Some(s) = updated.sprites.get_mut(sprite_index) {
                        s.path = basename;
                    }
                    draft.set(updated);
                    // webview の HTTP キャッシュ無効化（同 basename 上書き / 過去キャッシュ対策）
                    if let Some(mut buster) = cache_buster {
                        buster.write().bump();
                    }
                    replace_error.set(None);
                }
                Err(e) => replace_error.set(Some(e.to_string())),
            }
        }
    };

    let mut on_pivot = move |axis: usize, evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<i32>() else {
            return;
        };
        let mut updated = draft();
        if let Some(s) = updated.sprites.get_mut(sprite_index) {
            // 値が同じなら no-op (履歴を増やさない)
            if s.pivot_point[axis] == v {
                return;
            }
            s.pivot_point[axis] = v;
        } else {
            return;
        }
        history.record();
        draft.set(updated);
    };

    let selected = selected_box();

    rsx! {
        div { class: "h-full overflow-y-auto p-3 space-y-4 bg-base-200 rounded-box",

            div {
                h3 { class: "font-semibold mb-2", "Sprite #{sprite.index}" }
                p { class: "text-xs font-mono text-base-content/60 break-all", "{sprite.path}" }
                button { class: "btn btn-outline btn-xs mt-1", onclick: on_replace,
                    "画像を差し替え"
                }
                if let Some(message) = replace_error() {
                    p { class: "text-error text-xs mt-1", "{message}" }
                }
            }

            div {
                h3 { class: "font-semibold mb-1", "Pivot" }
                div { class: "flex gap-2 items-center",
                    label { class: "text-xs", "x" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-20",
                        value: "{pivot[0]}",
                        onchange: move |evt| on_pivot(0, evt),
                    }
                    label { class: "text-xs", "y" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-20",
                        value: "{pivot[1]}",
                        onchange: move |evt| on_pivot(1, evt),
                    }
                }
            }

            BoxList {
                kind: BoxKind::Body,
                boxes: sprite.body_boxes.clone().unwrap_or_default(),
                selected,
                selected_box,
            }
            BoxList {
                kind: BoxKind::Attack,
                // AttackBox の hitbox 部分のみを並べる (meta は別 UI で編集予定)。
                boxes: sprite
                    .attack_boxes
                    .as_deref()
                    .map(|v| v.iter().map(|ab| ab.hitbox.clone()).collect())
                    .unwrap_or_default(),
                selected,
                selected_box,
            }

            if let Some(target) = selected {
                div { class: "border-t border-base-300 pt-3",
                    h3 { class: "font-semibold mb-2", "Selected Box" }
                    BoxEditor {
                        target,
                        sprite: sprite.clone(),
                        sprite_index,
                        character_depth: character.depth,
                        draft,
                        history,
                        selected_box,
                    }
                }
            }

            div { class: "border-t border-base-300 pt-3",
                ReferenceSection { character: character.clone(), references }
            }
        }
    }
}

#[component]
fn BoxList(
    kind: BoxKind,
    boxes: Vec<HitBox>,
    selected: Option<SelectedBox>,
    mut selected_box: Signal<Option<SelectedBox>>,
) -> Element {
    rsx! {
        div {
            h3 { class: "font-semibold mb-1", "{kind.list_heading()} ({boxes.len()})" }
            if boxes.is_empty() {
                p { class: "text-xs text-base-content/60 italic", "なし" }
            } else {
                div { class: "flex flex-col gap-1",
                    for (i, hb) in boxes.iter().enumerate() {
                        BoxListItem {
                            key: "{i}",
                            kind,
                            index: i,
                            hitbox: hb.clone(),
                            is_selected: selected == Some(kind.select(i)),
                            selected_box,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn BoxListItem(
    kind: BoxKind,
    index: usize,
    hitbox: HitBox,
    is_selected: bool,
    mut selected_box: Signal<Option<SelectedBox>>,
) -> Element {
    let tl = hitbox.top_left();
    let br = hitbox.bottom_right();
    let w = hitbox.width();
    let h = hitbox.height();
    let row_class = if is_selected {
        "flex items-center gap-2 px-2 py-1 rounded text-left w-full text-xs bg-base-300 ring-1 ring-warning"
    } else {
        "flex items-center gap-2 px-2 py-1 rounded text-left w-full text-xs hover:bg-base-300"
    };
    let label = format!("{}{index}", kind.label_prefix());
    let badge_class = kind.list_badge_classes();

    rsx! {
        button {
            class: "{row_class}",
            onclick: move |_| selected_box.set(Some(kind.select(index))),
            span { class: "{badge_class}", "{label}" }
            span { class: "font-mono text-base-content/70", "[{tl[0]},{tl[1]}] - [{br[0]},{br[1]}]" }
            span { class: "ml-auto font-mono text-base-content/50", "{w}×{h}" }
        }
    }
}

#[component]
fn BoxEditor(
    target: SelectedBox,
    sprite: Sprite,
    sprite_index: usize,
    character_depth: u32,
    mut draft: Signal<SpriteGroup>,
    mut history: UseHistory<SpriteGroup>,
    mut selected_box: Signal<Option<SelectedBox>>,
) -> Element {
    let Some(hb) = sprite.get_box(target) else {
        return rsx! {
            p { class: "text-sm text-error italic", "選択中の Box が見つかりません。" }
        };
    };
    let tl = hb.top_left();
    let br = hb.bottom_right();
    let depth = hb.depth();
    let kind = target.kind();
    let label = format!("{} #{}", kind.singular_label(), target.index());
    let badge_class = kind.list_badge_classes();

    // 1 座標を更新して新しい HitBox を構築する (HitBox::new で正規化される)。
    // 値が変わらないなら履歴を増やさない (onchange は同値でも飛んでくる)。
    let on_corner = use_callback(move |(corner, value): (HitBoxCorner, i32)| {
        let mut updated = draft();
        let Some(s) = updated.sprites.get_mut(sprite_index) else {
            return;
        };
        let Some(current) = s.get_box(target) else {
            return;
        };
        let new_box = current.with_corner(corner, value);
        if new_box == current {
            return;
        }
        s.replace_box(target, new_box);
        history.record();
        draft.set(updated);
    });

    // depth (Option<u32>) の差し替え。HitBoxDepthInput が同値判定済みで呼ぶ前提。
    let on_depth = use_callback(move |new_depth: Option<u32>| {
        let mut updated = draft();
        let Some(s) = updated.sprites.get_mut(sprite_index) else {
            return;
        };
        let Some(current) = s.get_box(target) else {
            return;
        };
        if current.depth() == new_depth {
            return;
        }
        s.replace_box(target, current.with_depth(new_depth));
        history.record();
        draft.set(updated);
    });

    let on_delete = move |_| {
        let mut updated = draft();
        let Some(s) = updated.sprites.get_mut(sprite_index) else {
            return;
        };
        s.remove_box(target);
        history.record();
        draft.set(updated);
        selected_box.set(None);
    };

    // Attack の場合のみ AttackBoxMeta 編集をネストして表示する。Body は meta が無い。
    let is_attack = matches!(target, SelectedBox::Attack(_));
    let attack_meta: Option<AttackBoxMeta> = if is_attack {
        sprite.get_attack_box(target.index()).and_then(|ab| ab.meta)
    } else {
        None
    };
    let on_attack_meta = use_callback(move |new_meta: Option<AttackBoxMeta>| {
        let mut updated = draft();
        let Some(s) = updated.sprites.get_mut(sprite_index) else {
            return;
        };
        s.replace_attack_meta(target.index(), new_meta);
        history.record();
        draft.set(updated);
    });

    let input_class = "input input-bordered input-sm w-full";
    rsx! {
        div { class: "space-y-2",
            div { class: "flex items-center gap-2",
                span { class: "{badge_class}", "{label}" }
            }
            div { class: "grid grid-cols-[auto_1fr_1fr] gap-2 items-center",
                span { class: "text-xs", "Top Left" }
                HitBoxCornerInput {
                    corner: HitBoxCorner::TopLeftX,
                    value: tl[0],
                    class: input_class,
                    on_change: move |v| on_corner.call((HitBoxCorner::TopLeftX, v)),
                }
                HitBoxCornerInput {
                    corner: HitBoxCorner::TopLeftY,
                    value: tl[1],
                    class: input_class,
                    on_change: move |v| on_corner.call((HitBoxCorner::TopLeftY, v)),
                }
                span { class: "text-xs", "Bottom Right" }
                HitBoxCornerInput {
                    corner: HitBoxCorner::BottomRightX,
                    value: br[0],
                    class: input_class,
                    on_change: move |v| on_corner.call((HitBoxCorner::BottomRightX, v)),
                }
                HitBoxCornerInput {
                    corner: HitBoxCorner::BottomRightY,
                    value: br[1],
                    class: input_class,
                    on_change: move |v| on_corner.call((HitBoxCorner::BottomRightY, v)),
                }
                span {
                    class: "text-xs",
                    title: "world Z 厚み (奥行き)。空欄で Character.depth にフォールバック",
                    "Depth (Z)"
                }
                div { class: "col-span-2",
                    HitBoxDepthInput {
                        current: depth,
                        fallback: character_depth,
                        class: input_class,
                        on_change: move |v| on_depth.call(v),
                    }
                }
            }
            if is_attack {
                // Attack の場合は AttackBoxMeta (Damage / KnockbackDamage / Knockback Vec3 /
                // HitStop) 編集を併設する。全 0 のときは None として保存される。
                div { class: "pt-2 border-t border-base-300",
                    h4 { class: "text-xs font-semibold uppercase tracking-wide text-base-content/70 mb-1",
                        "Attack Meta"
                    }
                    AttackMetaInputs {
                        meta: attack_meta,
                        on_change: move |v| on_attack_meta.call(v),
                    }
                }
            }
            button { class: "btn btn-error btn-outline btn-xs", onclick: on_delete, "Delete Box" }
        }
    }
}
