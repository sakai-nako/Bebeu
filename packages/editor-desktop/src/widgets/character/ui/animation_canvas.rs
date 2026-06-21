use dioxus::prelude::*;

use super::canvas_common::{
    DragState as CommonDragState, EditorBoxOverlay, client_xy, delta_zoomed, is_primary_click,
    next_wheel_zoom, pan_start_payload, pan_to_screen, partition_references,
};
use super::canvas_visibility::{CanvasVisibility, CanvasVisibilityBar, Field};
use super::sprite_reference::{ReferenceLayer, SpriteReference};
use crate::entities::character::{
    Animation, BoxKind, Character, Frame, Layer, SelectedBox, use_playback,
};
use crate::entities::preference::use_preferences;
use crate::shared::{
    FlipMode, HitBox, ResizeHandle, UseHistory, ViewControlBindings, use_image_cache_buster,
    versioned_asset_url, workspace_asset_url,
};

type DragState = CommonDragState<DragKind>;

/// 編集 layer の image 左上（=画像内 (0,0) が来る canvas-内座標）を返す。
/// LayerView と Inherit box overlay の双方が同じ式を使う。
fn layer_image_offset(
    sprite_pivot: [i32; 2],
    frame_offset: [i32; 2],
    layer_offset: [i32; 2],
) -> [i32; 2] {
    [
        -sprite_pivot[0] + frame_offset[0] + layer_offset[0],
        -sprite_pivot[1] + frame_offset[1] + layer_offset[1],
    ]
}

#[derive(Debug, Clone, PartialEq)]
enum DragKind {
    /// Canvas 全体のパン。delta は zoom 補正なしでそのまま pan に加算。
    PanCanvas { start_pan: [f64; 2] },
    /// Frame Pivot Offset marker のドラッグ。delta は image-pixel 単位で frame.pivot_point_offset に加算。
    MovePivotOffset { start_offset: [i32; 2] },
    /// Layer Pivot Offset (Sprite pivot marker) のドラッグ。layer は frame.layers 内の `layer.index`
    /// で識別する (frame.layers Vec の物理順序は不定なので index で探す)。
    MoveLayerPivotOffset {
        layer_index: u32,
        start_offset: [i32; 2],
    },
    /// Override box の平行移動。
    MoveOverrideBox { target: SelectedBox, start: HitBox },
    /// Override box のリサイズ。掴んだハンドル (4 隅 + 4 辺中点) で動かす座標が決まる。
    ResizeOverrideBox {
        handle: ResizeHandle,
        target: SelectedBox,
        start: HitBox,
    },
}

/// drag を frame に適用する。PanCanvas は呼び出し側で事前に分岐される。
/// pivot_point_offset / Override box はいずれも state (= Frame.Flip 適用前) 座標で書き戻すため、
/// 画面 delta (dx, dy) を `invert_drag_delta` で逆 flip してから加算/translate/resize に渡す。
/// これで Frame.Flip 適用後の見た目とマウス移動方向が一致する。
fn apply_frame_drag(frame: &mut Frame, kind: &DragKind, dx: i32, dy: i32) {
    let (sdx, sdy) = invert_drag_delta(frame.flip, dx, dy);
    match kind {
        DragKind::MovePivotOffset { start_offset } => {
            let next = [start_offset[0] + sdx, start_offset[1] + sdy];
            // 全 0 のときは None に正規化（YAML の null と一致、Panel 側のロジックと揃える）
            frame.pivot_point_offset = if next == [0, 0] { None } else { Some(next) };
        }
        DragKind::MoveLayerPivotOffset {
            layer_index,
            start_offset,
        } => {
            if let Some(layer) = frame.layers.iter_mut().find(|l| l.index == *layer_index) {
                let next = [start_offset[0] + sdx, start_offset[1] + sdy];
                layer.pivot_point_offset = if next == [0, 0] { None } else { Some(next) };
            }
        }
        DragKind::MoveOverrideBox { target, start } => {
            frame.replace_override_box(*target, start.translated(sdx, sdy));
        }
        DragKind::ResizeOverrideBox {
            handle,
            target,
            start,
        } => {
            frame.replace_override_box(*target, start.resized(*handle, sdx, sdy));
        }
        DragKind::PanCanvas { .. } => unreachable!("PanCanvas は呼び出し側で事前に分岐される"),
    }
}

/// 選択 Frame の Layer を pivot 揃えで重ね描きするプレビュー。
/// ホイールでズーム、preferences で指定したマウスボタンでパン。
/// Pivot Offset / Body / Attack box は drag/resize で編集可能。
/// Body / Attack の override が `None` のときは各 Layer の sprite から box を継承表示する。
#[component]
pub fn AnimationCanvas(
    character: Character,
    draft: Signal<Animation>,
    history: UseHistory<Animation>,
    selected_frame_index: ReadSignal<usize>,
    /// 選択中 Layer の物理 index (= renumber_layers 済みなので layer.index と一致)。
    /// 各 layer の画像本体 (image bounds) を click で選択 + 同 click から drag で
    /// その layer の `pivot_point_offset` を編集する。Signal なので書き込みも行う。
    selected_layer_index: Signal<Option<usize>>,
    selected_box: Signal<Option<SelectedBox>>,
    references: ReadSignal<Vec<SpriteReference>>,
    visibility: Signal<CanvasVisibility>,
) -> Element {
    let preferences = use_preferences();
    let bindings: ViewControlBindings = preferences.read().view_controls;
    let vis = visibility();
    // 再生中は drag 系編集を無効化する。pan / zoom / 選択は許可するので、各 mousedown で peek して
    // 早期 return するパターンにする (Canvas 全体に pointer-events-none を当てない)。
    let playback = use_playback();

    let zoom = use_signal(|| 1.0_f64);
    let pan = use_signal(|| [0.0_f64, 0.0_f64]);
    let dragging = use_signal(|| None::<DragState>);

    let frame_index = selected_frame_index();
    let frame = {
        let read = draft.read();
        read.frames.get(frame_index).cloned()
    };
    let Some(frame) = frame else {
        return rsx! {
            div { class: "h-full flex items-center justify-center text-base-content/60 italic",
                "Frame を選択してください。"
            }
        };
    };

    // layer.index 昇順で描画（小さい index が背景、大きい index が前景）
    let mut layers = frame.layers.clone();
    layers.sort_by_key(|l| l.index);

    // Reference を Back / Front に分配
    let (back_refs, front_refs) = partition_references(&references());

    let frame_offset = frame.pivot_point_offset.unwrap_or([0, 0]);
    let frame_flip = frame.flip;
    let layers_is_empty = layers.is_empty();
    let frame_index_label = frame.index;
    let frame_ticks_label = frame.ticks;

    let zoom_value = zoom();
    let pan_value = pan();
    let pan_x = pan_value[0];
    let pan_y = pan_value[1];

    // ドラッグ中の操作対象 (mousemove では dragging を変えないので reactive read は mousedown/up でしか発火しない)
    let drag_kind = dragging().map(|d| d.kind);
    let is_frame_pivot_drag = matches!(drag_kind, Some(DragKind::MovePivotOffset { .. }));
    let active_layer_pivot: Option<u32> = match &drag_kind {
        Some(DragKind::MoveLayerPivotOffset { layer_index, .. }) => Some(*layer_index),
        _ => None,
    };
    let is_layer_pivot_drag = active_layer_pivot.is_some();
    let active_box_target: Option<SelectedBox> = match &drag_kind {
        Some(
            DragKind::MoveOverrideBox { target, .. } | DragKind::ResizeOverrideBox { target, .. },
        ) => Some(*target),
        _ => None,
    };
    let any_box_drag = active_box_target.is_some();
    // 「pivot 系の drag が起きている = 他の pivot/box は dim する」用のまとめ判定。
    let any_pivot_drag = is_frame_pivot_drag || is_layer_pivot_drag;

    // Canvas root の mousedown: pan bind は最優先、そうでなければ primary click で Frame Pivot
    // Offset drag を開始する。LayerView / Box overlay / Reference 等の上では各 overlay が
    // stop_propagation するので、ここに届くのは「空白部分の primary click」だけ → そのまま
    // Frame Pivot Offset 編集ハンドルとして機能する (marker 自身は表示のみ)。
    let on_canvas_mousedown = {
        let mut selected_box = selected_box;
        let mut dragging = dragging;
        let mut history = history;
        move |evt: MouseEvent| {
            if let Some((start_pan, start_mouse)) = pan_start_payload(&evt, bindings, *pan.peek()) {
                dragging.set(Some(DragState::new(
                    DragKind::PanCanvas { start_pan },
                    start_mouse,
                )));
                return;
            }
            if !is_primary_click(&evt) {
                return;
            }
            // 再生中は編集 drag を開始させない (pan は許可済み)
            if playback.peek().locks_editing() {
                return;
            }
            // 空白 primary click → Frame Pivot Offset drag を開始。box 選択も一緒に解除。
            selected_box.set(None);
            history.record();
            dragging.set(Some(DragState::new(
                DragKind::MovePivotOffset {
                    start_offset: frame_offset,
                },
                client_xy(&evt),
            )));
        }
    };

    let on_canvas_mousemove = {
        let mut draft = draft;
        let mut pan = pan;
        move |evt: MouseEvent| {
            let Some(drag) = dragging.peek().as_ref().cloned() else {
                return;
            };
            let mouse = client_xy(&evt);

            if let DragKind::PanCanvas { start_pan } = drag.kind {
                pan.set(pan_to_screen(start_pan, drag.start_mouse, mouse));
                return;
            }

            // image-pixel 系で扱うため zoom で割る
            let z = *zoom.peek();
            let dx = delta_zoomed(mouse[0] - drag.start_mouse[0], z);
            let dy = delta_zoomed(mouse[1] - drag.start_mouse[1], z);

            // start_mouse からの累積を直接適用するので dx==0 && dy==0 でも frame は変わらない。
            // ただし frame 単位で比較してから set することで余計な再 render を抑える。
            let new_frame = {
                let read = draft.read();
                let Some(f) = read.frames.get(frame_index) else {
                    return;
                };
                let mut next = f.clone();
                apply_frame_drag(&mut next, &drag.kind, dx, dy);
                next
            };
            let same = draft.peek().frames.get(frame_index) == Some(&new_frame);
            if !same {
                let mut updated = draft.peek().clone();
                if let Some(slot) = updated.frames.get_mut(frame_index) {
                    *slot = new_frame;
                    draft.set(updated);
                }
            }
        }
    };

    let reset_drag = {
        let mut dragging = dragging;
        move |_evt: MouseEvent| dragging.set(None)
    };

    let on_wheel = {
        let mut zoom = zoom;
        move |evt: WheelEvent| {
            let current = *zoom.peek();
            if let Some(next) = next_wheel_zoom(&evt, bindings, current) {
                zoom.set(next);
            }
        }
    };

    // 4K + 150% (= 非整数 DPR) で画像が device pixel grid に subpixel で乗らないよう、
    // wrapper には `transform: scale(zoom)` を使わず、各子要素の CSS 寸法を image-pixel × zoom で
    // explicit に書く方針に揃える (SpriteCanvas と同じ。詳細は ui/README.md)。
    //
    // marker (Origin / Frame Pivot / Layer Pivot) は viewport wrapper の child 座標で:
    //   - Origin = (zoom/2, zoom/2) ← frame_offset = 0 の pivot 画素中央
    //   - Frame Pivot = (frame_offset * zoom + zoom/2, ...)
    //   - Layer Pivot (per layer) = ((frame_offset + layer_offset) * zoom + zoom/2, ...)
    // で配置し、`transform: translate(-50%, -50%)` で各 SVG 要素を中央寄せする。
    let marker_center_offset = zoom_value / 2.0;
    let frame_pivot_marker_x = f64::from(frame_offset[0]) * zoom_value + marker_center_offset;
    let frame_pivot_marker_y = f64::from(frame_offset[1]) * zoom_value + marker_center_offset;
    let pivot_fill_class = if is_frame_pivot_drag {
        "fill-warning"
    } else {
        "fill-primary"
    };
    // Frame Pivot は Layer Pivot drag や Box drag のときに dim する (主役を譲る)
    let pivot_wrapper_opacity = if is_layer_pivot_drag || any_box_drag {
        "opacity-40"
    } else {
        ""
    };

    // Override / Inherit のどちらを描くかを Body/Attack それぞれに判定。
    let body_overrides = frame.body_box_overrides.clone();
    let attack_overrides = frame.attack_box_overrides.clone();
    let body_is_inherit = body_overrides.is_none();
    let attack_is_inherit = attack_overrides.is_none();

    let selected = selected_box();

    rsx! {
        div {
            class: "relative w-full h-full overflow-hidden checkerboard-bg select-none",
            onmousedown: on_canvas_mousedown,
            onmousemove: on_canvas_mousemove,
            onmouseup: reset_drag,
            onmouseleave: reset_drag,
            onwheel: on_wheel,

            // viewport wrapper (= pan アンカー)。
            // - 0×0 で left/top: round(50%, 1px) → canvas 中央。child (0, 0) は frame-pivot 画素の左上。
            // - transform: translate(pan) のみ (zoom は子要素の CSS px に直接乗算する)。
            // - will-change: transform で GPU compositing layer をヒント。
            // - SpriteCanvas と同じ「explicit pixel sizing」方針 (4K + 150% subpixel 揺れ対策)。
            div {
                class: "absolute",
                style: "left: round(50%, 1px); top: round(50%, 1px); width: 0; height: 0; transform: translate({pan_x}px, {pan_y}px); will-change: transform;",

                if layers_is_empty {
                    div {
                        class: "absolute pointer-events-none text-base-content/60 italic text-sm whitespace-nowrap",
                        style: "left: 0; top: 0; transform: translate(-50%, -50%);",
                        "Layer がありません。"
                    }
                }

                // Back references: child (0, 0) が frame-pivot 画素左上なので、各 reference は
                // 自身の pivot ぶん戻して描く。ReferenceLayer に zoom を渡して explicit sizing。
                if vis.references {
                    for (i, reference) in back_refs.iter().enumerate() {
                        div {
                            key: "ref-back-{i}",
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

                // Layers: explicit sizing 構成で各 layer 自身が zoom を扱う。
                // 元画像外枠 (vis.image_frame) は各 layer の image にぴったり重ねるため LayerView 内で描く。
                // 各 layer の image bounds は click で選択 + 同 click から drag で
                // `MoveLayerPivotOffset` を開始する (= sprite pivot dot marker は表示専用に降格)。
                // Box overlay は後段で描画されるため CSS rendering order で前に来る → Box 上では
                // Box が pointer events を奪い、自然に「Box の上はドラッグ無効」が成立する。
                for layer in layers.iter() {
                    LayerView {
                        key: "{layer.index}",
                        character: character.clone(),
                        layer: layer.clone(),
                        frame_offset,
                        frame_flip,
                        zoom: zoom_value,
                        show_frame: vis.image_frame,
                        is_drag_active: active_layer_pivot == Some(layer.index),
                        selected_layer_index,
                        history,
                        dragging,
                    }
                }

                // Inherit boxes (override が None の系統だけ、各 layer の box を read-only で重ねる)
                if vis.body_boxes && body_is_inherit {
                    for layer in layers.iter() {
                        InheritLayerBoxes {
                            key: "inherit-body-{layer.index}",
                            kind: BoxKind::Body,
                            layer: layer.clone(),
                            character: character.clone(),
                            frame_offset,
                            frame_flip,
                            zoom: zoom_value,
                            dimmed: any_pivot_drag,
                        }
                    }
                }
                if vis.attack_boxes && attack_is_inherit {
                    for layer in layers.iter() {
                        InheritLayerBoxes {
                            key: "inherit-attack-{layer.index}",
                            kind: BoxKind::Attack,
                            layer: layer.clone(),
                            character: character.clone(),
                            frame_offset,
                            frame_flip,
                            zoom: zoom_value,
                            dimmed: any_pivot_drag,
                        }
                    }
                }

                // Override boxes (interactive)
                if vis.body_boxes {
                    if let Some(boxes) = body_overrides.as_ref() {
                        for (i, hb) in boxes.iter().enumerate() {
                            OverrideBoxOverlay {
                                key: "ov-body-{i}",
                                target: SelectedBox::Body(i),
                                hitbox: hb.clone(),
                                frame_offset,
                                frame_flip,
                                zoom: zoom_value,
                                is_selected: selected == Some(SelectedBox::Body(i)),
                                dimmed: any_pivot_drag
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    || (any_box_drag && active_box_target != Some(SelectedBox::Body(i))),
                                history,
                                dragging,
                                selected_box,
                            }
                        }
                    }
                }
                if vis.attack_boxes {
                    if let Some(boxes) = attack_overrides.as_ref() {
                        for (i, ab) in boxes.iter().enumerate() {
                            // partial inherit: hitbox=None の場合は sprite[i] の hitbox を
                            // 各 layer の sprite から取り出して dashed 描画 (= 全 box Inherit
                            // モードと同じ見た目で「ここは sprite から継承中」を伝える)。
                            // hitbox=Some の場合は通常通り interactive な OverrideBoxOverlay。
                            if let Some(hb) = ab.hitbox.clone() {
                                OverrideBoxOverlay {
                                    key: "ov-attack-{i}",
                                    target: SelectedBox::Attack(i),
                                    hitbox: hb,
                                    frame_offset,
                                    frame_flip,
                                    zoom: zoom_value,
                                    is_selected: selected == Some(SelectedBox::Attack(i)),
                                    dimmed: any_pivot_drag
                                        || (any_box_drag
                                            && active_box_target != Some(SelectedBox::Attack(i))),
                                    history,
                                    dragging,
                                    selected_box,
                                }
                            } else {
                                for layer in layers.iter() {
                                    PartialInheritAttackBox {
                                        key: "partial-inherit-attack-{i}-{layer.index}",
                                        layer: layer.clone(),
                                        character: character.clone(),
                                        box_index: i,
                                        frame_offset,
                                        frame_flip,
                                        zoom: zoom_value,
                                        dimmed: any_pivot_drag,
                                    }
                                }
                            }
                        }
                    }
                }

                // Front references
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

                // Canvas origin marker: frame_offset = [0, 0] のときに Frame Pivot Offset marker が
                // 来る位置 (= viewport の child (0, 0) の画素中央)。viewport wrapper 内なので pan は
                // 自動で追従する。SVG で subpixel 位置でも一貫した描画になる。
                if vis.origin {
                    svg {
                        class: "absolute pointer-events-none fill-base-content/60",
                        style: "left: {marker_center_offset}px; top: {marker_center_offset}px; transform: translate(-50%, -50%); overflow: visible;",
                        width: "20",
                        height: "20",
                        view_box: "0 0 20 20",
                        "aria-label": "Canvas origin (frame_offset = 0)",
                        rect {
                            x: "9",
                            y: "0",
                            width: "2",
                            height: "20",
                        }
                        rect {
                            x: "0",
                            y: "9",
                            width: "20",
                            height: "2",
                        }
                        circle { cx: "10", cy: "10", r: "3" }
                    }
                }

                // Frame Pivot Offset marker: 表示専用 (drag は Canvas root が拾う)。
                // pointer-events-none で透過させ、Canvas 空白 drag を妨げない。drag 中だけ
                // fill が warning に切り替わる (Layer Pivot drag / Box drag 中は dim)。
                if vis.frame_pivot {
                    svg {
                        class: "absolute pointer-events-none {pivot_wrapper_opacity}",
                        style: "left: {frame_pivot_marker_x}px; top: {frame_pivot_marker_y}px; transform: translate(-50%, -50%); overflow: visible;",
                        width: "28",
                        height: "28",
                        view_box: "0 0 28 28",
                        "aria-label": "Frame Pivot Offset (canvas の空白を drag で移動)",
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
                    }
                }

                // Sprite pivot markers: 表示専用 (drag は LayerView の image overlay が拾う)。
                // viewport wrapper 内に置くので pan は自動追従。各 layer の
                // (frame_offset + layer_offset) * zoom + zoom/2 が pivot 画素中央 CSS。
                if vis.layer_pivot {
                    for layer in layers.iter() {
                        SpritePivotMarker {
                            key: "sprite-pivot-{layer.index}",
                            child_x: f64::from(frame_offset[0] + layer.pivot_point_offset.unwrap_or([0, 0])[0])
                                * zoom_value
                                + marker_center_offset,
                            child_y: f64::from(frame_offset[1] + layer.pivot_point_offset.unwrap_or([0, 0])[1])
                                * zoom_value
                                + marker_center_offset,
                            layer_index: layer.index,
                            is_active: active_layer_pivot == Some(layer.index),
                            dimmed: is_frame_pivot_drag
                                || any_box_drag
                                || (is_layer_pivot_drag && active_layer_pivot != Some(layer.index)),
                        }
                    }
                }
            }

            // フレーム情報 + 可視性トグルのオーバーレイ（pan の外側で常に左上）。
            // CanvasVisibilityBar は session 内のみで永続化しない（references と同じ扱い）。
            div { class: "absolute top-2 left-2 flex items-center gap-2",
                div { class: "badge badge-neutral badge-sm font-mono",
                    "Frame #{frame_index_label} · {frame_ticks_label} tick"
                }
                CanvasVisibilityBar {
                    visibility,
                    fields: vec![
                        Field::FramePivot,
                        Field::LayerPivot,
                        Field::BodyBoxes,
                        Field::AttackBoxes,
                        Field::References,
                        Field::Origin,
                        Field::ImageFrame,
                    ],
                }
            }
        }
    }
}

#[component]
fn LayerView(
    character: Character,
    layer: Layer,
    frame_offset: [i32; 2],
    frame_flip: Option<FlipMode>,
    zoom: f64,
    /// 元画像の外枠 (image dimensions の矩形) を image にぴったり重ねて描くか。
    show_frame: bool,
    /// この layer の Layer Pivot drag が進行中か。cursor を grabbing にするだけに使う。
    is_drag_active: bool,
    /// 共有の selected layer signal。image overlay の click で `Some(layer.index)` を書く。
    selected_layer_index: Signal<Option<usize>>,
    history: UseHistory<Animation>,
    dragging: Signal<Option<DragState>>,
) -> Element {
    let playback = use_playback();
    let resolved = character.find_sprite(layer.sprite_group_number, layer.sprite_index);
    let Some((group, sprite)) = resolved else {
        return rsx! {
            div {
                class: "absolute rounded border border-error/60 bg-error/10 text-[10px] text-error font-mono px-1 pointer-events-none",
                style: "left: 0; top: 0; transform: translate(-50%, -50%);",
                title: "SpriteGroup #{layer.sprite_group_number}, sprite_index {layer.sprite_index} が見つかりません",
                "missing #{layer.sprite_group_number}/{layer.sprite_index}"
            }
        };
    };

    // SpriteGroupEditor で画像差し替え後に WebView の HTTP キャッシュが古い画像を返すのを
    // 防ぐため、`?v={N}` を付与して再フェッチを促す。
    let cache_buster = use_image_cache_buster();
    let version = cache_buster.map_or(0, |s| s.read().0);
    let url = versioned_asset_url(
        workspace_asset_url(&format!(
            "data/characters/{}/sprite-groups/{}/sprites/{}",
            character.name, group.name, sprite.path,
        )),
        version,
    );

    let layer_offset = layer.pivot_point_offset.unwrap_or([0, 0]);
    let [dx, dy] = layer_image_offset(sprite.pivot_point, frame_offset, layer_offset);
    // viewport wrapper の child 座標系で image の左上に来る位置 (zoom-multiplied CSS px)。
    let dx_zoomed = f64::from(dx) * zoom;
    let dy_zoomed = f64::from(dy) * zoom;
    // image の表示サイズ。dimensions が None なら 0 にして非表示 fallback。
    let (img_w_zoomed, img_h_zoomed) = sprite.dimensions.map_or((0.0_f64, 0.0_f64), |[w, h]| {
        (f64::from(w) * zoom, f64::from(h) * zoom)
    });
    // Frame.Flip の transform-origin は image-pixel coord の frame_offset (= frame_pivot 画素の左上) を
    // CSS px に変換した値。viewport child 座標系の (0, 0) は image-pixel coord [0, 0] の画素左上なので
    // pivot は viewport child から見て frame_offset * zoom の位置になる。
    let frame_pivot_origin_x = f64::from(frame_offset[0]) * zoom;
    let frame_pivot_origin_y = f64::from(frame_offset[1]) * zoom;
    // Layer.Flip の transform-origin は image-pixel coord の sprite.pivot_point を CSS px に変換した値。
    let sprite_pivot_origin_x = f64::from(sprite.pivot_point[0]) * zoom;
    let sprite_pivot_origin_y = f64::from(sprite.pivot_point[1]) * zoom;

    // Frame.Flip は wrapper div に当てる (transform-origin = frame_pivot 画素の左上)。
    // Layer.Flip は img 自身に当てる (transform-origin = sprite.pivot 画素の左上)。
    // Inherit/Override box overlay でも対応する flipped_around 適用と座標的に整合する。
    let frame_scale = flip_scale_css(frame_flip);
    let layer_scale = flip_scale_css(layer.flip);

    let opacity = layer.transparency.clamp(0.0, 1.0);

    // image bounds に overlay div を常時被せて、click=layer 選択 + drag=Layer Pivot Offset 編集
    // を兼用する。dimensions = None (image 解決不能) のときは width/height = 0 なので実質発火しない。
    let layer_index = layer.index;
    let start_offset = layer.pivot_point_offset.unwrap_or([0, 0]);
    let on_image_mousedown = {
        let mut dragging = dragging;
        let mut history = history;
        let mut selected_layer_index = selected_layer_index;
        move |evt: MouseEvent| {
            if !is_primary_click(&evt) {
                return;
            }
            if playback.peek().locks_editing() {
                return;
            }
            // Canvas root の Frame Pivot drag を抑止 + Box selection 維持。
            evt.stop_propagation();
            // 未選択ならまずこの layer を選択 (drag せず離してもこの選択は残る)。
            // 選択済みのときは set 自体が同値で no-op (= 再 render しない)。
            selected_layer_index.set(Some(layer_index as usize));
            history.record();
            dragging.set(Some(DragState::new(
                DragKind::MoveLayerPivotOffset {
                    layer_index,
                    start_offset,
                },
                client_xy(&evt),
            )));
        }
    };
    let overlay_cursor = if is_drag_active {
        "cursor-grabbing"
    } else {
        "cursor-grab"
    };

    rsx! {
        div {
            class: "absolute pointer-events-none",
            style: "left: 0; top: 0; opacity: {opacity}; transform: {frame_scale}; transform-origin: {frame_pivot_origin_x}px {frame_pivot_origin_y}px;",

            img {
                src: "{url}",
                alt: "layer #{layer.index}",
                class: "absolute block",
                draggable: false,
                style: "left: {dx_zoomed}px; top: {dy_zoomed}px; width: {img_w_zoomed}px; height: {img_h_zoomed}px; max-width: none; transform: {layer_scale}; transform-origin: {sprite_pivot_origin_x}px {sprite_pivot_origin_y}px; image-rendering: pixelated;",
            }

            // 元画像の外枠: image と同じ left/top/width/height/transform で box-border の枠線だけを重ねる。
            // dimensions = None のときは img_*_zoomed = 0 で 0x0 となり実質非表示。
            if show_frame {
                div {
                    class: "absolute pointer-events-none box-border border border-dashed border-base-content/60",
                    style: "left: {dx_zoomed}px; top: {dy_zoomed}px; width: {img_w_zoomed}px; height: {img_h_zoomed}px; transform: {layer_scale}; transform-origin: {sprite_pivot_origin_x}px {sprite_pivot_origin_y}px;",
                }
            }

            // image bounds の click/drag 拾い overlay (常時)。click で layer 選択 + 同 click から
            // drag で Layer Pivot Offset 編集。img と同じ位置・transform で重ねる
            // (Box overlay は後段で描かれるので CSS rendering 順で前に来る → Box 上は Box が拾う)。
            div {
                class: "absolute pointer-events-auto {overlay_cursor}",
                style: "left: {dx_zoomed}px; top: {dy_zoomed}px; width: {img_w_zoomed}px; height: {img_h_zoomed}px; transform: {layer_scale}; transform-origin: {sprite_pivot_origin_x}px {sprite_pivot_origin_y}px;",
                onmousedown: on_image_mousedown,
            }
        }
    }
}

/// FlipMode を CSS `transform: scale(...)` 文字列に変換する。`None` は変換なし。
fn flip_scale_css(flip: Option<FlipMode>) -> &'static str {
    match flip {
        Some(FlipMode::Horizontal) => "scale(-1, 1)",
        Some(FlipMode::Vertical) => "scale(1, -1)",
        Some(FlipMode::Both) => "scale(-1, -1)",
        None => "none",
    }
}

/// 各 Layer の sprite から body / attack box を取り出して read-only で描画する。
/// Override が None (Inherit) のときだけマウントされる。
/// Layer.Flip と Frame.Flip は `resolve_inherit_box_in_frame` で box 座標を反転してから描画する
/// (LayerView の CSS transform と幾何的に整合)。
#[component]
fn InheritLayerBoxes(
    kind: BoxKind,
    layer: Layer,
    character: Character,
    frame_offset: [i32; 2],
    frame_flip: Option<FlipMode>,
    zoom: f64,
    dimmed: bool,
) -> Element {
    let Some((_group, sprite)) =
        character.find_sprite(layer.sprite_group_number, layer.sprite_index)
    else {
        return rsx! {};
    };
    let Some(boxes) = kind.sprite_hitbox_slice(sprite) else {
        return rsx! {};
    };

    let layer_offset = layer.pivot_point_offset.unwrap_or([0, 0]);

    let color_class = kind.inherit_box_classes();
    let label_prefix = kind.label_prefix();
    // Inherit box の border/bg 自体が `border-info/70 bg-info/10` のような alpha 付きクラスなので、
    // ここで更に opacity-60 を被せると 70% × 60% = 42% で見えなくなりがち。
    // wrapping は dim 時 (opacity-50) のみで、通常は alpha クラスの色をそのまま出す。
    let opacity_class = if dimmed { "opacity-50" } else { "" };

    rsx! {
        for (i, hb) in boxes.iter().enumerate() {
            InheritBoxOverlay {
                key: "{i}",
                hitbox: resolve_inherit_box_in_frame(
                    hb,
                    sprite.pivot_point,
                    layer_offset,
                    layer.flip,
                    frame_flip,
                ),
                frame_offset,
                zoom,
                color_class: color_class.to_string(),
                opacity_class: opacity_class.to_string(),
                label: format!("{label_prefix}{i}"),
            }
        }
    }
}

/// `AttackBoxOverride.hitbox == None` (= sprite から hitbox 継承中、partial inherit) の
/// box について、各 layer の sprite から該当 index の hitbox を取り出して dashed 描画する。
/// `InheritLayerBoxes` の attack-1-box 限定版。box 数 / layer 構成によっては該当 box が
/// 存在しないこともあり (sprite に対応 index の box が無い場合)、その場合は何も描画しない。
#[component]
fn PartialInheritAttackBox(
    layer: Layer,
    character: Character,
    box_index: usize,
    frame_offset: [i32; 2],
    frame_flip: Option<FlipMode>,
    zoom: f64,
    dimmed: bool,
) -> Element {
    let Some((_group, sprite)) =
        character.find_sprite(layer.sprite_group_number, layer.sprite_index)
    else {
        return rsx! {};
    };
    let Some(boxes) = sprite.attack_boxes.as_ref() else {
        return rsx! {};
    };
    let Some(ab) = boxes.get(box_index) else {
        return rsx! {};
    };

    let layer_offset = layer.pivot_point_offset.unwrap_or([0, 0]);
    let kind = BoxKind::Attack;
    let color_class = kind.inherit_box_classes();
    let label_prefix = kind.label_prefix();
    let opacity_class = if dimmed { "opacity-50" } else { "" };

    rsx! {
        InheritBoxOverlay {
            hitbox: resolve_inherit_box_in_frame(
                &ab.hitbox,
                sprite.pivot_point,
                layer_offset,
                layer.flip,
                frame_flip,
            ),
            frame_offset,
            zoom,
            color_class: color_class.to_string(),
            opacity_class: opacity_class.to_string(),
            label: format!("{label_prefix}{box_index}"),
        }
    }
}

#[component]
fn InheritBoxOverlay(
    /// frame 座標系 (frame_pivot 中心) で flip 解決済みの hitbox。
    hitbox: HitBox,
    frame_offset: [i32; 2],
    zoom: f64,
    color_class: String,
    opacity_class: String,
    label: String,
) -> Element {
    let tl = hitbox.top_left();
    let abs_x = f64::from(frame_offset[0] + tl[0]) * zoom;
    let abs_y = f64::from(frame_offset[1] + tl[1]) * zoom;
    let w = f64::from(hitbox.width().max(1)) * zoom;
    let h = f64::from(hitbox.height().max(1)) * zoom;
    rsx! {
        div {
            // border-2 + dashed で「override box (solid 2px) と区別がつく read-only 表示」を維持しつつ、
            // zoom 非依存でも視認できる太さにする (旧 1px は zoom 倍されて見えてた)。
            class: "absolute pointer-events-none border-2 border-dashed {color_class} {opacity_class}",
            style: "left: {abs_x}px; top: {abs_y}px; width: {w}px; height: {h}px;",
            span {
                class: "absolute leading-none px-1 py-0.5 font-mono text-base-content/70",
                style: "left: 0; top: 0; font-size: 10px;",
                "{label}"
            }
        }
    }
}

#[component]
fn OverrideBoxOverlay(
    target: SelectedBox,
    /// state 上の hitbox (Frame.Flip 適用前)。描画と編集 delta の起点に使われる。
    hitbox: HitBox,
    frame_offset: [i32; 2],
    frame_flip: Option<FlipMode>,
    zoom: f64,
    is_selected: bool,
    dimmed: bool,
    mut history: UseHistory<Animation>,
    mut dragging: Signal<Option<DragState>>,
    mut selected_box: Signal<Option<SelectedBox>>,
) -> Element {
    let playback = use_playback();

    // 表示は Frame.Flip 適用後 (frame 座標で frame_pivot 中心の反転) の hitbox。
    // state 自体は反転前を保持し、編集 delta は `invert_drag_delta` / `invert_resize_handle`
    // で逆変換して未反転座標に投影する。
    let display_hitbox = maybe_flipped_around(&hitbox, [0, 0], frame_flip);
    let tl = display_hitbox.top_left();
    // box 配置: viewport wrapper の child 座標 (frame_offset + box.top_left) * zoom CSS px。
    let abs_x = f64::from(frame_offset[0] + tl[0]) * zoom;
    let abs_y = f64::from(frame_offset[1] + tl[1]) * zoom;
    let w = f64::from(display_hitbox.width().max(1)) * zoom;
    let h = f64::from(display_hitbox.height().max(1)) * zoom;
    let position_style = format!("left: {abs_x}px; top: {abs_y}px; width: {w}px; height: {h}px;");

    let on_start_move = {
        let hitbox = hitbox.clone();
        move |evt: MouseEvent| {
            if !is_primary_click(&evt) {
                return;
            }
            // 再生中は box drag を開始させない
            if playback.peek().locks_editing() {
                return;
            }
            evt.stop_propagation();
            selected_box.set(Some(target));
            history.record();
            dragging.set(Some(DragState::new(
                DragKind::MoveOverrideBox {
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
            if playback.peek().locks_editing() {
                return;
            }
            evt.stop_propagation();
            selected_box.set(Some(target));
            history.record();
            // 画面上のハンドル (= flip 後の見た目) を state 用 (= 反転前) に逆 flip。
            // 例: Frame.Flip=H で画面右辺ハンドルを掴むと、state 上は左辺ハンドルを動かす。
            let state_handle = invert_resize_handle(frame_flip, handle);
            dragging.set(Some(DragState::new(
                DragKind::ResizeOverrideBox {
                    handle: state_handle,
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
            hitbox: display_hitbox,
            position_style,
            is_selected,
            dimmed,
            on_start_move,
            on_start_resize,
        }
    }
}

/// 各 Layer の Sprite pivot を canvas 上に示すマーカー (表示専用)。
/// drag による pivot offset 編集は LayerView の image overlay が拾うため、ここは pointer-events-none。
/// is_active = この layer の Layer Pivot drag が進行中、で fill を warning に切り替えるのみ。
///
/// 4K (高 DPI) ディスプレイで subpixel 位置が device pixel grid に合わず、frame ごとに
/// ブラウザが違う方向に pixel-snap して見える問題を避けるため、SVG で描く。
#[component]
fn SpritePivotMarker(
    /// viewport wrapper の child 座標における marker 中心の (x, y) CSS px。
    /// 呼び出し側で `(frame_offset + layer_offset) * zoom + zoom/2` を渡す。
    child_x: f64,
    child_y: f64,
    layer_index: u32,
    is_active: bool,
    dimmed: bool,
) -> Element {
    let fill_class = if is_active {
        "fill-warning"
    } else {
        "fill-secondary"
    };
    let opacity_class = if dimmed { "opacity-40" } else { "" };

    rsx! {
        svg {
            class: "absolute pointer-events-none {opacity_class}",
            style: "left: {child_x}px; top: {child_y}px; transform: translate(-50%, -50%); overflow: visible;",
            width: "10",
            height: "10",
            view_box: "0 0 10 10",
            "aria-label": "Layer #{layer_index} pivot offset",
            // dot 本体: 10x10 outer、6x6 fill 内側、2px base-100 ring
            circle {
                class: "{fill_class} stroke-base-100",
                cx: "5",
                cy: "5",
                r: "4",
                stroke_width: "2",
            }
        }
    }
}

/// `flip` が `Some` のときだけ `HitBox::flipped_around` を適用する小ヘルパー。
fn maybe_flipped_around(hb: &HitBox, pivot: [i32; 2], flip: Option<FlipMode>) -> HitBox {
    match flip {
        Some(mode) => hb.flipped_around(pivot, mode),
        None => hb.clone(),
    }
}

/// Inherit box (= sprite-pixel 座標) を「Layer.Flip → sprite-pixel → frame 座標 → Frame.Flip」の順に
/// 解決して、frame 座標系の HitBox を返す。frame 座標は frame_pivot 画素の左上が原点 [0, 0]。
///
/// LayerView の CSS transform 階層 (img: Layer.Flip / wrapper: Frame.Flip) と幾何的に等価。
fn resolve_inherit_box_in_frame(
    sprite_box: &HitBox,
    sprite_pivot: [i32; 2],
    layer_offset: [i32; 2],
    layer_flip: Option<FlipMode>,
    frame_flip: Option<FlipMode>,
) -> HitBox {
    let local = maybe_flipped_around(sprite_box, sprite_pivot, layer_flip);
    let in_frame = local.translated(
        -sprite_pivot[0] + layer_offset[0],
        -sprite_pivot[1] + layer_offset[1],
    );
    maybe_flipped_around(&in_frame, [0, 0], frame_flip)
}

/// 画面 (= flip 後) 座標系の delta (dx, dy) を state (= 反転前) 座標系に逆変換する。
/// `HitBox::flipped_around` が involution であるのと同様、delta も同じ flip を当てれば往復する。
fn invert_drag_delta(flip: Option<FlipMode>, dx: i32, dy: i32) -> (i32, i32) {
    match flip {
        Some(FlipMode::Horizontal) => (-dx, dy),
        Some(FlipMode::Vertical) => (dx, -dy),
        Some(FlipMode::Both) => (-dx, -dy),
        None => (dx, dy),
    }
}

/// resize ハンドルを Frame.Flip で反転させる。画面上「右辺」を掴んでも state では「左辺」を動かす、
/// という対応関係を生む。Horizontal なら Left ↔ Right、Vertical なら Top ↔ Bottom、Both なら両方。
fn invert_resize_handle(flip: Option<FlipMode>, handle: ResizeHandle) -> ResizeHandle {
    let Some(mode) = flip else {
        return handle;
    };
    let (mut h, mut v) = match handle {
        ResizeHandle::TopLeft => (Some(false), Some(false)),
        ResizeHandle::Top => (None, Some(false)),
        ResizeHandle::TopRight => (Some(true), Some(false)),
        ResizeHandle::Left => (Some(false), None),
        ResizeHandle::Right => (Some(true), None),
        ResizeHandle::BottomLeft => (Some(false), Some(true)),
        ResizeHandle::Bottom => (None, Some(true)),
        ResizeHandle::BottomRight => (Some(true), Some(true)),
    };
    let (flip_h, flip_v) = match mode {
        FlipMode::Horizontal => (true, false),
        FlipMode::Vertical => (false, true),
        FlipMode::Both => (true, true),
    };
    if flip_h {
        h = h.map(|x| !x);
    }
    if flip_v {
        v = v.map(|x| !x);
    }
    match (h, v) {
        (Some(false), Some(false)) => ResizeHandle::TopLeft,
        (None, Some(false)) => ResizeHandle::Top,
        (Some(true), Some(false)) => ResizeHandle::TopRight,
        (Some(false), None) => ResizeHandle::Left,
        (Some(true), None) => ResizeHandle::Right,
        (Some(false), Some(true)) => ResizeHandle::BottomLeft,
        (None, Some(true)) => ResizeHandle::Bottom,
        (Some(true), Some(true)) => ResizeHandle::BottomRight,
        (None, None) => unreachable!("ResizeHandle has at least one axis"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_drag_delta_handles_each_flip_mode() {
        assert_eq!(invert_drag_delta(None, 3, 5), (3, 5));
        assert_eq!(invert_drag_delta(Some(FlipMode::Horizontal), 3, 5), (-3, 5));
        assert_eq!(invert_drag_delta(Some(FlipMode::Vertical), 3, 5), (3, -5));
        assert_eq!(invert_drag_delta(Some(FlipMode::Both), 3, 5), (-3, -5));
    }

    #[test]
    fn invert_drag_delta_is_involution() {
        // 同じ flip を 2 回当てると元に戻る (flipped_around と同じ性質)。
        for mode in [
            None,
            Some(FlipMode::Horizontal),
            Some(FlipMode::Vertical),
            Some(FlipMode::Both),
        ] {
            let (dx, dy) = (7, -11);
            let (a, b) = invert_drag_delta(mode, dx, dy);
            assert_eq!(invert_drag_delta(mode, a, b), (dx, dy));
        }
    }

    #[test]
    fn invert_resize_handle_horizontal_swaps_left_right() {
        let h = Some(FlipMode::Horizontal);
        assert_eq!(
            invert_resize_handle(h, ResizeHandle::Left),
            ResizeHandle::Right
        );
        assert_eq!(
            invert_resize_handle(h, ResizeHandle::Right),
            ResizeHandle::Left
        );
        assert_eq!(
            invert_resize_handle(h, ResizeHandle::TopLeft),
            ResizeHandle::TopRight
        );
        assert_eq!(
            invert_resize_handle(h, ResizeHandle::BottomRight),
            ResizeHandle::BottomLeft
        );
        // 縦中央のハンドル (Top / Bottom) は Vertical 軸を持たないので H では不変。
        assert_eq!(
            invert_resize_handle(h, ResizeHandle::Top),
            ResizeHandle::Top
        );
        assert_eq!(
            invert_resize_handle(h, ResizeHandle::Bottom),
            ResizeHandle::Bottom
        );
    }

    #[test]
    fn invert_resize_handle_vertical_swaps_top_bottom() {
        let v = Some(FlipMode::Vertical);
        assert_eq!(
            invert_resize_handle(v, ResizeHandle::Top),
            ResizeHandle::Bottom
        );
        assert_eq!(
            invert_resize_handle(v, ResizeHandle::Bottom),
            ResizeHandle::Top
        );
        assert_eq!(
            invert_resize_handle(v, ResizeHandle::TopLeft),
            ResizeHandle::BottomLeft
        );
        assert_eq!(
            invert_resize_handle(v, ResizeHandle::Left),
            ResizeHandle::Left
        );
    }

    #[test]
    fn invert_resize_handle_both_inverts_all_corners() {
        let b = Some(FlipMode::Both);
        assert_eq!(
            invert_resize_handle(b, ResizeHandle::TopLeft),
            ResizeHandle::BottomRight
        );
        assert_eq!(
            invert_resize_handle(b, ResizeHandle::TopRight),
            ResizeHandle::BottomLeft
        );
        assert_eq!(
            invert_resize_handle(b, ResizeHandle::Top),
            ResizeHandle::Bottom
        );
        assert_eq!(
            invert_resize_handle(b, ResizeHandle::Left),
            ResizeHandle::Right
        );
    }

    #[test]
    fn invert_resize_handle_none_is_identity() {
        for handle in [
            ResizeHandle::TopLeft,
            ResizeHandle::Top,
            ResizeHandle::TopRight,
            ResizeHandle::Left,
            ResizeHandle::Right,
            ResizeHandle::BottomLeft,
            ResizeHandle::Bottom,
            ResizeHandle::BottomRight,
        ] {
            assert_eq!(invert_resize_handle(None, handle), handle);
        }
    }

    #[test]
    fn resolve_inherit_box_in_frame_no_flip_just_translates_to_frame_coords() {
        // sprite-pixel [10..20, 5..15], sprite_pivot=[8, 6], layer_offset=[3, 2]
        // frame 座標 = box - sprite_pivot + layer_offset = [5..15, 1..11]
        let hb = HitBox::new(10, 5, 20, 15);
        let resolved = resolve_inherit_box_in_frame(&hb, [8, 6], [3, 2], None, None);
        assert_eq!(resolved.top_left(), [5, 1]);
        assert_eq!(resolved.bottom_right(), [15, 11]);
    }

    #[test]
    fn resolve_inherit_box_in_frame_layer_h_flips_around_sprite_pivot() {
        // sprite-pixel [10..20], sprite_pivot=[8, 6], H flip → [-4..6] (= 2*8 - 20, 2*8 - 10)
        // 続けて translate by (-sprite_pivot + layer_offset) = (-5, -4)
        // → frame 座標 [-9..1, ...]
        let hb = HitBox::new(10, 5, 20, 15);
        let resolved =
            resolve_inherit_box_in_frame(&hb, [8, 6], [3, 2], Some(FlipMode::Horizontal), None);
        assert_eq!(resolved.top_left()[0], -9);
        assert_eq!(resolved.bottom_right()[0], 1);
        // Y は反転しないので no-flip と同じ
        assert_eq!(resolved.top_left()[1], 1);
        assert_eq!(resolved.bottom_right()[1], 11);
    }

    #[test]
    fn resolve_inherit_box_in_frame_frame_h_flips_around_frame_pivot() {
        // 上の no-flip ケース: frame 座標 [5..15, 1..11]
        // Frame.Flip=H → frame_pivot ([0,0]) 中心に反転 → X [-15..-5]
        let hb = HitBox::new(10, 5, 20, 15);
        let resolved =
            resolve_inherit_box_in_frame(&hb, [8, 6], [3, 2], None, Some(FlipMode::Horizontal));
        assert_eq!(resolved.top_left()[0], -15);
        assert_eq!(resolved.bottom_right()[0], -5);
    }

    #[test]
    fn resolve_inherit_box_in_frame_layer_and_frame_h_compose() {
        // Layer.Flip=H → sprite-pivot 中心反転 → sprite-pixel [-4..6]
        // frame 座標化 → [-9..1]
        // Frame.Flip=H → [0, 0] 中心反転 → [-1..9]
        let hb = HitBox::new(10, 5, 20, 15);
        let resolved = resolve_inherit_box_in_frame(
            &hb,
            [8, 6],
            [3, 2],
            Some(FlipMode::Horizontal),
            Some(FlipMode::Horizontal),
        );
        assert_eq!(resolved.top_left()[0], -1);
        assert_eq!(resolved.bottom_right()[0], 9);
    }
}
