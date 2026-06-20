use std::borrow::Cow;

use dioxus::prelude::*;

use super::super::attack_meta_inputs::AttackMetaInputs;
use super::super::hitbox_inputs::{HitBoxCornerInput, HitBoxDepthInput};
use super::{flip_to_value, parse_flip};
use crate::entities::character::{Animation, BoxKind, SelectedBox};
use crate::shared::{AttackBoxMeta, FlipMode, HitBox, HitBoxCorner, UseHistory};

/// Frame override が新規 Override モードに切り替わった時に最初に置く box の初期サイズ。
/// 16x16 px の小さな矩形なので、ユーザーが座標を編集する際の出発点として使う。
const DEFAULT_OVERRIDE_BOX_SIZE: i32 = 16;

fn default_override_box() -> HitBox {
    HitBox::new(0, 0, DEFAULT_OVERRIDE_BOX_SIZE, DEFAULT_OVERRIDE_BOX_SIZE)
}

/// Frame の body / attack box override の 3 状態。
/// データ表現は `Option<Vec<HitBox>>` で (→ ADR-0014):
///   - `Inherit` = `None`           → Sprite の box をそのまま使う
///   - `Override` = `Some(non-empty)` → Frame で box を上書き
///   - `Disable` = `Some(empty)`     → Sprite の box を無効化（box 無し）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoxOverrideMode {
    Inherit,
    Override,
    Disable,
}

impl BoxOverrideMode {
    /// `BoxKind::frame_override_state` の返値 (`Option<usize>`) から 3 状態を判定する。
    /// Body / Attack で Vec の要素型が違っても、長さだけ見れば 3 状態は決まる。
    fn from_state(state: Option<usize>) -> Self {
        match state {
            None => Self::Inherit,
            Some(0) => Self::Disable,
            Some(_) => Self::Override,
        }
    }
}

/// Frame レベルで Sprite の値を上書きする 3 系統 (Pivot Offset / Body Box / Attack Box) を
/// 1 セクションにまとめる。Pivot Offset は加算的な offset で厳密には override ではないが、
/// UI 的には「Frame 単位で見え方を調整する設定」として同居させる。
///
/// `character_depth` は HitBox.depth が None のとき UI に "(inherit)" として表示する
/// フォールバック値。BoxRow の depth 入力に渡る (ADR-0024)。
#[component]
pub(super) fn FrameOverridesSection(
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    character_depth: u32,
    flip: Option<FlipMode>,
    pivot_offset: Option<[i32; 2]>,
    // Body / Attack の override は型が違うので、3 状態判定に使う「box 個数」だけを
    // 親から渡す (`None`=Inherit / `Some(0)`=Disable / `Some(n>0)`=Override)。実データ
    // (HitBox / AttackBox) は子 component が `BoxKind` 経由で改めて取り直す。
    body_state: Option<usize>,
    attack_state: Option<usize>,
    selected_box: Signal<Option<SelectedBox>>,
) -> Element {
    let on_flip = move |evt: Event<FormData>| {
        let new_flip = parse_flip(&evt.value());
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        if f.flip == new_flip {
            return;
        }
        f.flip = new_flip;
        history.record();
        draft.set(updated);
    };

    let off_x = pivot_offset.map_or(0, |p| p[0]);
    let off_y = pivot_offset.map_or(0, |p| p[1]);

    let mut on_offset = move |axis: usize, evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<i32>() else {
            return;
        };
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let mut current = f.pivot_point_offset.unwrap_or([0, 0]);
        if current[axis] == v {
            return;
        }
        current[axis] = v;
        // 全 0 のときは None に正規化（YAML の null と一致）
        f.pivot_point_offset = if current == [0, 0] {
            None
        } else {
            Some(current)
        };
        history.record();
        draft.set(updated);
    };

    let flip_value = flip_to_value(flip);

    rsx! {
        div { class: "space-y-3",
            h3 { class: "font-semibold", "Overrides" }

            div { class: "space-y-2",
                h4 { class: "text-xs font-semibold uppercase tracking-wide text-base-content/70",
                    "Flip"
                }
                select {
                    class: "select select-bordered select-sm w-32",
                    value: "{flip_value}",
                    onchange: on_flip,
                    option { value: "none", "None" }
                    option { value: "horizontal", "Horizontal" }
                    option { value: "vertical", "Vertical" }
                    option { value: "both", "Both" }
                }
            }

            div { class: "space-y-2",
                h4 { class: "text-xs font-semibold uppercase tracking-wide text-base-content/70",
                    "Pivot Offset"
                }
                div { class: "flex items-center gap-1",
                    span { class: "text-xs", "x" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-16",
                        value: "{off_x}",
                        onchange: move |evt| on_offset(0, evt),
                    }
                    span { class: "text-xs", "y" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-16",
                        value: "{off_y}",
                        onchange: move |evt| on_offset(1, evt),
                    }
                }
            }

            BoxOverrideSection {
                target: BoxKind::Body,
                draft,
                history,
                frame_index,
                character_depth,
                state: body_state,
                selected_box,
            }

            BoxOverrideSection {
                target: BoxKind::Attack,
                draft,
                history,
                frame_index,
                character_depth,
                state: attack_state,
                selected_box,
            }
        }
    }
}

#[component]
fn BoxOverrideSection(
    target: BoxKind,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    character_depth: u32,
    /// override box の個数だけ (3 状態判定用)。Body は HitBox、Attack は AttackBox の
    /// 個数を渡される。実 box の中身は子 BoxRow が `BoxKind` 経由で取り直す。
    state: Option<usize>,
    selected_box: Signal<Option<SelectedBox>>,
) -> Element {
    let title = target.singular_label();
    let mode = BoxOverrideMode::from_state(state);
    let radio_name = format!("override-mode-{frame_index}-{}", target.id());

    // モード変更: Inherit/Override/Disable のいずれかを選んで slot を書き換える。
    // Override に切り替える際、既存値が空または None なら default box を 1 つ入れて
    // Override の意味を保つ（空のままだと Disable と区別がつかなくなる）。
    let on_mode_change = use_callback(move |new_mode: BoxOverrideMode| {
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let current = BoxOverrideMode::from_state(target.frame_override_state(f));
        if current == new_mode {
            return;
        }
        match new_mode {
            BoxOverrideMode::Inherit => target.set_frame_override_inherit(f),
            BoxOverrideMode::Override => {
                target.ensure_frame_override_present(f, default_override_box());
            }
            BoxOverrideMode::Disable => target.set_frame_override_disable(f),
        }
        history.record();
        draft.set(updated);
    });

    let on_add_box = move |_| {
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        target.push_frame_override_box(f, default_override_box());
        history.record();
        draft.set(updated);
    };

    // 現在の box 群 (HitBox スライス相当) を draft から取り出す。Override モードのみで使う。
    let current_hitboxes: Vec<HitBox> = draft()
        .frames
        .get(frame_index)
        .and_then(|f| target.frame_override_hitbox_slice(f).map(Cow::into_owned))
        .unwrap_or_default();

    rsx! {
        div { class: "space-y-2",
            h4 { class: "text-xs font-semibold uppercase tracking-wide text-base-content/70",
                "{title}"
            }
            div { class: "flex flex-col gap-1",
                label { class: "label cursor-pointer justify-start gap-2 py-0",
                    input {
                        r#type: "radio",
                        class: "radio radio-xs",
                        name: "{radio_name}",
                        checked: mode == BoxOverrideMode::Inherit,
                        onchange: move |_| on_mode_change.call(BoxOverrideMode::Inherit),
                    }
                    span { class: "label-text text-xs", "上書きしない" }
                }
                label { class: "label cursor-pointer justify-start gap-2 py-0",
                    input {
                        r#type: "radio",
                        class: "radio radio-xs",
                        name: "{radio_name}",
                        checked: mode == BoxOverrideMode::Override,
                        onchange: move |_| on_mode_change.call(BoxOverrideMode::Override),
                    }
                    span { class: "label-text text-xs", "上書きする" }
                }
                label { class: "label cursor-pointer justify-start gap-2 py-0",
                    input {
                        r#type: "radio",
                        class: "radio radio-xs",
                        name: "{radio_name}",
                        checked: mode == BoxOverrideMode::Disable,
                        onchange: move |_| on_mode_change.call(BoxOverrideMode::Disable),
                    }
                    span { class: "label-text text-xs", "上書きする (Sprite の box を無効化)" }
                }
            }

            if mode == BoxOverrideMode::Override {
                div { class: "space-y-1 ml-4",
                    for (i, hit_box) in current_hitboxes.iter().enumerate() {
                        BoxRow {
                            key: "{i}",
                            box_index: i,
                            hit_box: hit_box.clone(),
                            draft,
                            history,
                            frame_index,
                            target,
                            character_depth,
                            selected_box,
                        }
                    }
                    button { class: "btn btn-outline btn-xs", onclick: on_add_box, "+ Add Box" }
                }
            }
        }
    }
}

#[component]
fn BoxRow(
    box_index: usize,
    hit_box: HitBox,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    target: BoxKind,
    character_depth: u32,
    mut selected_box: Signal<Option<SelectedBox>>,
) -> Element {
    let tl = hit_box.top_left();
    let br = hit_box.bottom_right();
    let depth = hit_box.depth();

    // 1 座標を更新して新しい HitBox を構築する (HitBox::new で正規化される)。
    // Attack の場合は AttackBox.hitbox 部分のみ差し替え、meta は `BoxKind` 側で保持される。
    let update_coord = use_callback(move |(corner, value): (HitBoxCorner, i32)| {
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let Some(current) = target.get_frame_override_hitbox(f, box_index) else {
            return;
        };
        let new_box = current.with_corner(corner, value);
        if current == new_box {
            return;
        }
        target.replace_frame_override_hitbox(f, box_index, new_box);
        history.record();
        draft.set(updated);
    });

    // depth (Option<u32>) の差し替え。HitBoxDepthInput が同値判定済みで呼ぶ前提。
    let update_depth = use_callback(move |new_depth: Option<u32>| {
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let Some(current) = target.get_frame_override_hitbox(f, box_index) else {
            return;
        };
        if current.depth() == new_depth {
            return;
        }
        target.replace_frame_override_hitbox(f, box_index, current.with_depth(new_depth));
        history.record();
        draft.set(updated);
    });

    let on_remove = move |_| {
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        target.remove_frame_override_box(f, box_index);
        history.record();
        draft.set(updated);
    };

    let is_selected = selected_box() == Some(target.select(box_index));
    let row_class = if is_selected {
        "flex items-center gap-1 text-xs px-1 rounded ring-1 ring-warning bg-warning/10 cursor-pointer"
    } else {
        "flex items-center gap-1 text-xs px-1 rounded hover:bg-base-300 cursor-pointer"
    };
    let on_select = move |_| selected_box.set(Some(target.select(box_index)));

    // Attack 系列のみ AttackBoxMeta 編集 UI を出す。Body には meta が無い。
    let is_attack = target == BoxKind::Attack;
    let attack_meta: Option<AttackBoxMeta> = if is_attack {
        draft()
            .frames
            .get(frame_index)
            .and_then(|f| f.get_attack_override(box_index))
            .and_then(|ab| ab.meta)
    } else {
        None
    };
    let update_attack_meta = use_callback(move |new_meta: Option<AttackBoxMeta>| {
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        f.replace_attack_override_meta(box_index, new_meta);
        history.record();
        draft.set(updated);
    });

    let input_class = "input input-bordered input-xs w-12";
    rsx! {
        div { class: "space-y-1",
            div { class: "{row_class}", onclick: on_select,
                span { class: "font-mono w-6 text-base-content/60", "#{box_index}" }
                HitBoxCornerInput {
                    corner: HitBoxCorner::TopLeftX,
                    value: tl[0],
                    class: input_class,
                    on_change: move |v| update_coord.call((HitBoxCorner::TopLeftX, v)),
                }
                HitBoxCornerInput {
                    corner: HitBoxCorner::TopLeftY,
                    value: tl[1],
                    class: input_class,
                    on_change: move |v| update_coord.call((HitBoxCorner::TopLeftY, v)),
                }
                span { class: "text-base-content/40", "→" }
                HitBoxCornerInput {
                    corner: HitBoxCorner::BottomRightX,
                    value: br[0],
                    class: input_class,
                    on_change: move |v| update_coord.call((HitBoxCorner::BottomRightX, v)),
                }
                HitBoxCornerInput {
                    corner: HitBoxCorner::BottomRightY,
                    value: br[1],
                    class: input_class,
                    on_change: move |v| update_coord.call((HitBoxCorner::BottomRightY, v)),
                }
                span {
                    class: "text-xs text-base-content/60 ml-1",
                    title: "world Z 厚み",
                    "Z"
                }
                HitBoxDepthInput {
                    current: depth,
                    fallback: character_depth,
                    class: input_class,
                    on_change: move |v| update_depth.call(v),
                }
                button {
                    class: "btn btn-ghost btn-xs text-error ml-auto",
                    title: "削除",
                    onclick: on_remove,
                    "✕"
                }
            }
            // Attack だけ meta (Damage/KnockbackDamage/HitstunExtra/Knockback Vec3) を編集できる。
            if is_attack {
                div { class: "ml-7",
                    AttackMetaInputs {
                        meta: attack_meta,
                        on_change: move |v| update_attack_meta.call(v),
                    }
                }
            }
        }
    }
}
