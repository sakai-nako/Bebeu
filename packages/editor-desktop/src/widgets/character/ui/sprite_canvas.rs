use dioxus::prelude::*;

use super::canvas_common::{
    DragState as CommonDragState, EditorBoxOverlay, client_xy, delta_zoomed, is_primary_click,
    next_wheel_zoom, pan_start_payload, pan_to_screen, partition_references,
};
use super::canvas_visibility::{CanvasVisibility, CanvasVisibilityBar, Field};
use super::sprite_reference::{ReferenceLayer, SpriteReference};
use crate::entities::character::{Character, SelectedBox, Sprite, SpriteGroup};
use crate::entities::preference::use_preferences;
use crate::shared::{
    HitBox, ResizeHandle, UseHistory, ViewControlBindings, use_image_cache_buster,
    versioned_asset_url, workspace_asset_url,
};

/// 親 (sprite_group_editor) が dragging signal の型として参照するためのエイリアス。
pub(crate) type DragState = CommonDragState<DragKind>;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DragKind {
    /// Sprite (画像 + HitBox 全体) のドラッグ。pivot_point を逆方向に更新する。
    MoveSprite {
        start_pivot: [i32; 2],
    },
    MoveBox {
        target: SelectedBox,
        start: HitBox,
    },
    ResizeBox {
        handle: ResizeHandle,
        target: SelectedBox,
        start: HitBox,
    },
    /// Canvas 全体のパン（視点移動）。`start_pan` は mousedown 時点で確定し、mousemove で
    /// canvas-pixel delta をそのまま（zoom 補正なしで）加算する。
    PanCanvas {
        start_pan: [f64; 2],
    },
}

/// drag の delta を sprite に適用した新しい Sprite を返す。
/// PanCanvas は呼び出し側で事前に分岐されるため、ここに来ない前提。
fn apply_sprite_drag(sprite: &Sprite, kind: &DragKind, dx: i32, dy: i32) -> Sprite {
    let mut new_sprite = sprite.clone();
    match kind {
        DragKind::MoveSprite { start_pivot } => {
            new_sprite.pivot_point = [start_pivot[0] - dx, start_pivot[1] - dy];
        }
        DragKind::MoveBox { target, start } => {
            new_sprite.replace_box(*target, start.translated(dx, dy));
        }
        DragKind::ResizeBox {
            handle,
            target,
            start,
        } => {
            new_sprite.replace_box(*target, start.resized(*handle, dx, dy));
        }
        DragKind::PanCanvas { .. } => unreachable!("PanCanvas は呼び出し側で事前に分岐される"),
    }
    new_sprite
}

#[component]
pub fn SpriteCanvas(
    character: Character,
    character_name: String,
    sprite_group_name: String,
    draft: Signal<SpriteGroup>,
    history: UseHistory<SpriteGroup>,
    selected_sprite_index: ReadSignal<usize>,
    selected_box: Signal<Option<SelectedBox>>,
    dragging: Signal<Option<DragState>>,
    references: ReadSignal<Vec<SpriteReference>>,
    visibility: Signal<CanvasVisibility>,
) -> Element {
    let preferences = use_preferences();
    let bindings: ViewControlBindings = preferences.read().view_controls;
    let vis = visibility();

    let zoom = use_signal(|| 1.0_f64);
    let pan = use_signal(|| [0.0_f64, 0.0_f64]);

    let sprite_index = selected_sprite_index();
    let sprite = {
        let read = draft.read();
        read.sprites.get(sprite_index).cloned()
    };
    let Some(sprite) = sprite else {
        return rsx! {
            div { class: "p-4 text-base-content/60 italic", "Sprite を選択してください。" }
        };
    };

    let cache_buster = use_image_cache_buster();
    let version = cache_buster.map_or(0, |s| s.read().0);
    let url = versioned_asset_url(
        workspace_asset_url(&format!(
            "data/characters/{character_name}/sprite-groups/{sprite_group_name}/sprites/{}",
            sprite.path
        )),
        version,
    );

    let pivot = sprite.pivot_point;
    let body_boxes: &[HitBox] = sprite.body_boxes.as_deref().unwrap_or(&[]);
    // AttackBox は hitbox 部分のみ描画する (meta は別 UI で編集)。canvas のループから
    // `&[HitBox]` 同等の挙動を保つため、参照だけのスライス形式で取り回す。
    let attack_hitboxes: Vec<HitBox> = sprite
        .attack_boxes
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|ab| ab.hitbox.clone())
        .collect();
    let attack_boxes: &[HitBox] = &attack_hitboxes;
    let selected = selected_box();

    // Reference を Back / Front に分配する。Back は画像の手前に DOM 配置するが、
    // 続く編集中 image / box overlay が後から描画されることで視覚的には奥になる。
    let (back_refs, front_refs) = partition_references(&references());

    let zoom_value = zoom();
    let pan_value = pan();
    let pan_x = pan_value[0];
    let pan_y = pan_value[1];

    // 4K + 150% (= 非整数 DPR) で画像が device pixel grid に subpixel で乗らないよう、
    // wrapper には `transform: scale(zoom)` を使わず、各子要素の CSS 寸法を
    // `image-pixel × zoom` で explicit に書く方針 (詳細は ui/README.md)。
    // ここで頻出の zoom-multiplied 値を pre-compute しておく。
    let pivot_x_zoomed = f64::from(pivot[0]) * zoom_value;
    let pivot_y_zoomed = f64::from(pivot[1]) * zoom_value;
    let neg_pivot_x_zoomed = -pivot_x_zoomed;
    let neg_pivot_y_zoomed = -pivot_y_zoomed;
    // image の width/height は Sprite.dimensions × zoom。dimensions = None (loader が PNG header
    // 読み損ねた等) の場合は 0 で fallback (= 画像非表示)。
    let (img_w_zoomed, img_h_zoomed) = sprite.dimensions.map_or((0.0_f64, 0.0_f64), |[w, h]| {
        (f64::from(w) * zoom_value, f64::from(h) * zoom_value)
    });
    // pivot marker の中心 "+" を「pivot 画素中央」に合わせるオフセット (= 半 image-pixel = zoom/2 CSS-px)。
    // wrapper の child 座標系で pivot pixel の左上が (0, 0) なので、中央は (zoom/2, zoom/2)。
    let marker_center_offset = zoom_value / 2.0;

    // ドラッグ中の操作対象を判定し、画像 / Box / Pivot の見た目に強調・減光を反映する。
    // mousemove では dragging を変更しないので、ここでの reactive read は mousedown / mouseup でしか発火しない。
    let drag_kind = dragging().map(|d| d.kind);
    let is_pivot_drag = matches!(drag_kind, Some(DragKind::MoveSprite { .. }));
    let active_box_target = match &drag_kind {
        Some(DragKind::MoveBox { target, .. } | DragKind::ResizeBox { target, .. }) => {
            Some(*target)
        }
        _ => None,
    };
    let any_box_drag = active_box_target.is_some();

    // 画像本体上で mousedown すると Pivot ドラッグを開始する (専用 marker と等価)。
    // BodyBox / AttackBox overlay は後段で stop_propagation しているので、Box の上では
    // ここに届かない (= 「Box 上はドラッグ無効」が自動で成立)。
    // Box ドラッグ中だけ少し dim させてフォーカスを譲る。
    // 編集 img は static にしておくこと。Tailwind preflight の `max-width: 100%` が、
    // wrapper が 0×0 だと max-width=0 となって img が消える。代わりに Back reference 側に
    // `z-index: -1` を付けることで、static の img より奥に描画させる。
    let img_class = if any_box_drag {
        "block opacity-70"
    } else if is_pivot_drag {
        "block cursor-grabbing"
    } else {
        "block cursor-grab"
    };

    // Pivot マーカーの色: MoveSprite 中は warning に切り替え、Box ドラッグ中は dim
    let pivot_fill_class = if is_pivot_drag {
        "fill-warning"
    } else {
        "fill-primary"
    };
    let pivot_wrapper_opacity = if any_box_drag { "opacity-40" } else { "" };
    let pivot_cursor_class = if is_pivot_drag {
        "cursor-grabbing"
    } else {
        "cursor-grab"
    };

    let on_canvas_mousedown = {
        let mut selected_box = selected_box;
        let mut dragging = dragging;
        move |evt: MouseEvent| {
            if let Some((start_pan, start_mouse)) = pan_start_payload(&evt, bindings, *pan.peek()) {
                dragging.set(Some(DragState::new(
                    DragKind::PanCanvas { start_pan },
                    start_mouse,
                )));
                return;
            }
            selected_box.set(None);
        }
    };

    let on_canvas_mousemove = {
        let mut draft = draft;
        let dragging = dragging;
        let mut pan = pan;
        move |evt: MouseEvent| {
            let Some(drag) = dragging.peek().as_ref().cloned() else {
                return;
            };
            let mouse = client_xy(&evt);
            if mouse == drag.start_mouse {
                return;
            }

            if let DragKind::PanCanvas { start_pan } = drag.kind {
                pan.set(pan_to_screen(start_pan, drag.start_mouse, mouse));
                return;
            }

            // image-pixel 系で扱うため zoom で割る
            let z = *zoom.peek();
            let dx = delta_zoomed(mouse[0] - drag.start_mouse[0], z);
            let dy = delta_zoomed(mouse[1] - drag.start_mouse[1], z);

            let new_sprite = {
                let read = draft.read();
                let Some(s) = read.sprites.get(sprite_index) else {
                    return;
                };
                apply_sprite_drag(s, &drag.kind, dx, dy)
            };
            let same = draft.peek().sprites.get(sprite_index) == Some(&new_sprite);
            if !same {
                let mut updated = draft.peek().clone();
                updated.sprites[sprite_index] = new_sprite;
                draft.set(updated);
            }
        }
    };

    let reset_drag = {
        let mut dragging = dragging;
        move |_evt: MouseEvent| dragging.set(None)
    };

    // Pivot drag を開始する共通ロジック。SVG marker / 画像本体の両方の mousedown で使う。
    let start_pivot_drag = {
        let mut dragging = dragging;
        let mut history = history;
        move |evt: MouseEvent| {
            if !is_primary_click(&evt) {
                return;
            }
            // Canvas root の mousedown が selected_box をクリアするのを止める。
            evt.stop_propagation();
            // drag 開始時点の draft をスナップショット。drag せず離した場合は Undo しても
            // 同値復元になるだけなので実害なし (UX 上のノイズもごく僅か)。
            history.record();
            dragging.set(Some(DragState::new(
                DragKind::MoveSprite { start_pivot: pivot },
                client_xy(&evt),
            )));
        }
    };
    // closure は Copy なので 2 箇所に渡しても move にならない (clippy::clone_on_copy 回避)。
    let on_pivot_mousedown = start_pivot_drag;
    let on_image_mousedown = start_pivot_drag;

    let on_wheel = {
        let mut zoom = zoom;
        move |evt: WheelEvent| {
            let current = *zoom.peek();
            if let Some(next) = next_wheel_zoom(&evt, bindings, current) {
                zoom.set(next);
            }
        }
    };

    rsx! {
        div {
            class: "relative w-full h-full overflow-hidden checkerboard-bg select-none",
            onmousedown: on_canvas_mousedown,
            onmousemove: on_canvas_mousemove,
            onmouseup: reset_drag,
            onmouseleave: reset_drag,
            onwheel: on_wheel,

            // viewport wrapper (= pivot 画素を canvas 中央に置くためのアンカー)。
            // - left/top: round(50%, 1px) で canvas 中央 (整数 CSS px)。
            // - width/height: 0 にして flex/transform 計算の基準点だけにする。
            // - transform: translate(pan) のみ。zoom は子要素の CSS px に直接乗算する方針なので、
            //   ここで scale(zoom) は使わない (4K + 150% で image bitmap が device grid に subpixel で
            //   乗って frame ごとに揺れるのを避けるため)。詳細は ui/README.md。
            // - will-change: transform で GPU compositing layer をヒント。
            // - 子要素の child 座標系 (0, 0) は「pivot 画素の左上」になるよう、各子で
            //   `(image-pixel - pivot) * zoom` を CSS px として書く。
            div {
                class: "absolute",
                style: "left: round(50%, 1px); top: round(50%, 1px); width: 0; height: 0; transform: translate({pan_x}px, {pan_y}px); will-change: transform;",

                // Back references: child (0, 0) が pivot 画素の左上なので、reference は自分で
                // 自分の pivot ぶん戻して描く (= ReferenceLayer 内部で `left: -ref.pivot_x*zoom` 等)。
                // CSS painting order の都合 (positioned > non-positioned) は image を `<img>` で
                // 出している限り存在するので、Back は z-index: -1 で奥に押し込む。
                if vis.references {
                    for (i, reference) in back_refs.iter().enumerate() {
                        div {
                            key: "ref-back-{i}",
                            class: "absolute",
                            style: "left: 0; top: 0; z-index: -1;",
                            ReferenceLayer {
                                character: character.clone(),
                                reference: reference.clone(),
                                zoom: zoom_value,
                            }
                        }
                    }
                }

                // 編集中 image: child (-pivot * zoom, -pivot * zoom) を左上にして width/height を
                // `naturalSize × zoom` の CSS px で explicit に置く。
                // browser はこの CSS box に合わせて image-rendering: pixelated の nearest-neighbor で
                // 1 step rasterize する (transform: scale だと 2 step compositing になり pixelated
                // が効かなくなることがある)。
                img {
                    src: "{url}",
                    class: "{img_class} absolute",
                    style: "image-rendering: pixelated; left: {neg_pivot_x_zoomed}px; top: {neg_pivot_y_zoomed}px; width: {img_w_zoomed}px; height: {img_h_zoomed}px; max-width: none;",
                    // ブラウザのネイティブ drag-and-drop を無効化する。
                    // 画像上で mousedown→dragstart が発火すると、進行中の Pivot / Box ドラッグでも
                    // mousemove がドラッグイベントに置き換わって我々のハンドラに届かなくなる。
                    draggable: false,
                    onmousedown: on_image_mousedown,
                }

                // 元画像の外枠: image と同じ位置・サイズに box-border の枠線だけを重ねる。
                // dimensions = None のときは img_*_zoomed = 0 で 0x0 となり実質非表示。
                if vis.image_frame {
                    div {
                        class: "absolute pointer-events-none box-border border border-dashed border-base-content/60",
                        style: "left: {neg_pivot_x_zoomed}px; top: {neg_pivot_y_zoomed}px; width: {img_w_zoomed}px; height: {img_h_zoomed}px;",
                    }
                }

                // Front references: 編集 image より後に描画されるので視覚的に手前に来る。
                // box overlay より先に描画することで、box 操作の pointer events を奪わない。
                if vis.references {
                    for (i, reference) in front_refs.iter().enumerate() {
                        div {
                            key: "ref-front-{i}",
                            class: "absolute",
                            style: "left: 0; top: 0;",
                            ReferenceLayer {
                                character: character.clone(),
                                reference: reference.clone(),
                                zoom: zoom_value,
                            }
                        }
                    }
                }

                if vis.body_boxes {
                    for (i, hb) in body_boxes.iter().enumerate() {
                        BoxOverlayWrapper {
                            key: "body-{i}",
                            hitbox: hb.clone(),
                            target: SelectedBox::Body(i),
                            is_selected: selected == Some(SelectedBox::Body(i)),
                            // Pivot ドラッグ中、または他 Box がドラッグされている時は dim
                            dimmed: is_pivot_drag || (any_box_drag && active_box_target != Some(SelectedBox::Body(i))),
                            pivot_x: pivot[0],
                            pivot_y: pivot[1],
                            zoom: zoom_value,
                            selected_box,
                            dragging,
                            history,
                        }
                    }
                }
                if vis.attack_boxes {
                    for (i, hb) in attack_boxes.iter().enumerate() {
                        BoxOverlayWrapper {
                            key: "attack-{i}",
                            hitbox: hb.clone(),
                            target: SelectedBox::Attack(i),
                            is_selected: selected == Some(SelectedBox::Attack(i)),
                            dimmed: is_pivot_drag || (any_box_drag && active_box_target != Some(SelectedBox::Attack(i))),
                            pivot_x: pivot[0],
                            pivot_y: pivot[1],
                            zoom: zoom_value,
                            selected_box,
                            dragging,
                            history,
                        }
                    }
                }

                // Pivot マーカー: viewport wrapper 内にいるので pan は wrapper transform 経由で適用される。
                // child 座標で pivot 画素の左上 (0, 0) から (zoom/2, zoom/2) ずらして「画素中央」に
                // marker の "+" を置く。SVG の内部 rasterizer が subpixel 位置でも一貫した anti-alias
                // 描画を行うので、4K + 150% でも揺れない。
                //
                // 画像上クリックでの Pivot 誤操作を防ぐため、Pivot 操作はこのマーカーに集約する。
                // MoveSprite ドラッグ中は warning 色、Box ドラッグ中は dim でフォーカスを譲る。
                if vis.pivot {
                    svg {
                        class: "absolute pointer-events-auto {pivot_cursor_class} {pivot_wrapper_opacity}",
                        style: "left: {marker_center_offset}px; top: {marker_center_offset}px; transform: translate(-50%, -50%); overflow: visible;",
                        width: "28",
                        height: "28",
                        view_box: "0 0 28 28",
                        onmousedown: on_pivot_mousedown,
                        rect {
                            class: "{pivot_fill_class}",
                            x: "12",
                            y: "0",
                            width: "4",
                            height: "28",
                        }
                        rect {
                            class: "{pivot_fill_class}",
                            x: "0",
                            y: "12",
                            width: "28",
                            height: "4",
                        }
                        circle {
                            class: "{pivot_fill_class} stroke-base-100",
                            cx: "14",
                            cy: "14",
                            r: "5",
                            stroke_width: "2",
                        }
                    }
                }
            }

            // 可視性トグルバー（pan の外側で常に左上）。CanvasVisibilityBar は
            // session 内のみで永続化しない（references と同じ扱い）。
            div { class: "absolute top-2 left-2",
                CanvasVisibilityBar {
                    visibility,
                    fields: vec![
                        Field::Pivot,
                        Field::BodyBoxes,
                        Field::AttackBoxes,
                        Field::References,
                        Field::ImageFrame,
                    ],
                }
            }
        }
    }
}

/// `EditorBoxOverlay` を SpriteCanvas 用にラップして、SpriteGroup 専用の DragKind を
/// dragging signal に書き込む。
///
/// `pivot_x` / `pivot_y` / `zoom` を受け取り、`(box.tl - pivot) * zoom` の CSS px で
/// explicit に位置/サイズを計算する (wrapper に scale(zoom) がない構成のため)。
#[component]
fn BoxOverlayWrapper(
    hitbox: HitBox,
    target: SelectedBox,
    is_selected: bool,
    dimmed: bool,
    pivot_x: i32,
    pivot_y: i32,
    zoom: f64,
    mut selected_box: Signal<Option<SelectedBox>>,
    mut dragging: Signal<Option<DragState>>,
    mut history: UseHistory<SpriteGroup>,
) -> Element {
    let tl = hitbox.top_left();
    let left = f64::from(tl[0] - pivot_x) * zoom;
    let top = f64::from(tl[1] - pivot_y) * zoom;
    let width = f64::from(hitbox.width().max(1)) * zoom;
    let height = f64::from(hitbox.height().max(1)) * zoom;
    let position_style =
        format!("left: {left}px; top: {top}px; width: {width}px; height: {height}px;");

    let on_start_move = {
        let hitbox = hitbox.clone();
        move |evt: MouseEvent| {
            if !is_primary_click(&evt) {
                return;
            }
            evt.stop_propagation();
            selected_box.set(Some(target));
            history.record();
            dragging.set(Some(DragState::new(
                DragKind::MoveBox {
                    target,
                    start: hitbox.clone(),
                },
                client_xy(&evt),
            )));
        }
    };

    let on_start_resize = {
        let hitbox = hitbox.clone();
        move |(handle, evt): (ResizeHandle, MouseEvent)| {
            if !is_primary_click(&evt) {
                return;
            }
            evt.stop_propagation();
            selected_box.set(Some(target));
            history.record();
            dragging.set(Some(DragState::new(
                DragKind::ResizeBox {
                    handle,
                    target,
                    start: hitbox.clone(),
                },
                client_xy(&evt),
            )));
        }
    };

    rsx! {
        EditorBoxOverlay {
            target,
            hitbox,
            position_style,
            is_selected,
            dimmed,
            on_start_move,
            on_start_resize,
        }
    }
}
