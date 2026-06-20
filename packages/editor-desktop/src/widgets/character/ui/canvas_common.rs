//! Sprite Canvas / Animation Canvas で共通する低レベル部品。
//!
//! 両 canvas は「pan + zoom + Body/Attack box overlay + Reference 重ね描画」という
//! 共通骨格を持ちつつ、編集対象が `Sprite` か `Frame` かで `DragKind` の variant が
//! 異なる。ここでは canvas-specific な DragKind に依存しない以下を集約する:
//!
//! - 座標変換 (`coord_to_px` / `delta_zoomed` / `client_xy`)
//! - クリック種別判定 (`is_primary_click`)
//! - PanCanvas / wheel zoom のロジック (`pan_start_payload` / `pan_to_screen` / `next_wheel_zoom`)
//! - Reference の Back/Front 分配 (`partition_references`)
//! - DragState<K> ラッパー (kind は canvas 側で定義)
//! - Body/Attack override box の overlay 描画 (`EditorBoxOverlay`)

use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;

use super::sprite_reference::{ReferencePlacement, SpriteReference};
use crate::entities::character::SelectedBox;
use crate::shared::{HitBox, ResizeHandle, ViewControlBindings};

/// Canvas の座標範囲は数千 px 程度で truncation の懸念はないので allow する。
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn coord_to_px(v: f64) -> i32 {
    v.round() as i32
}

/// canvas-pixel 単位の delta を image-pixel 単位 (zoom で割った値) へ変換する。
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn delta_zoomed(diff_px: i32, zoom: f64) -> i32 {
    let z = if zoom.abs() < f64::EPSILON { 1.0 } else { zoom };
    (f64::from(diff_px) / z).round() as i32
}

/// MouseEvent から client 座標（ビューポート基準 px）を i32 で取り出す。
/// element_coordinates はターゲットごとに座標系が変わるので、ターゲット非依存の
/// client_coordinates を使う。
pub(crate) fn client_xy(evt: &MouseEvent) -> [i32; 2] {
    let c = evt.client_coordinates();
    [coord_to_px(c.x), coord_to_px(c.y)]
}

/// `trigger_button` が Primary または不明（None）なら true。pan などの非 Primary 操作と
/// 区別するためのガード。
pub(crate) fn is_primary_click(evt: &MouseEvent) -> bool {
    evt.trigger_button()
        .is_none_or(|b| b == MouseButton::Primary)
}

/// マウスドラッグ中の操作種別と開始時点のスナップショット。canvas ごとに異なる
/// `DragKind` を type parameter で受け取る。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DragState<K> {
    pub kind: K,
    /// ドラッグ開始時のマウス座標（client 座標、ビューポート基準 px）。
    pub start_mouse: [i32; 2],
}

impl<K> DragState<K> {
    pub fn new(kind: K, start_mouse: [i32; 2]) -> Self {
        Self { kind, start_mouse }
    }
}

/// Pan ドラッグ開始用の payload。`mousedown` 時に bindings の pan_button と一致した
/// 場合のみ `Some((start_pan, start_mouse))` を返す。caller はこれを自分の DragKind
/// (`PanCanvas { start_pan }`) に詰めて dragging signal に書き込む。
pub(crate) fn pan_start_payload(
    evt: &MouseEvent,
    bindings: ViewControlBindings,
    current_pan: [f64; 2],
) -> Option<([f64; 2], [i32; 2])> {
    let button = evt.trigger_button()?;
    if !bindings.is_pan_button(button) {
        return None;
    }
    Some((current_pan, client_xy(evt)))
}

/// Pan ドラッグ中の `mousemove` で次の pan 値を計算する。zoom 補正は行わず、
/// canvas-pixel delta をそのまま加算する。
pub(crate) fn pan_to_screen(
    start_pan: [f64; 2],
    start_mouse: [i32; 2],
    current_mouse: [i32; 2],
) -> [f64; 2] {
    let dpx = current_mouse[0] - start_mouse[0];
    let dpy = current_mouse[1] - start_mouse[1];
    [start_pan[0] + f64::from(dpx), start_pan[1] + f64::from(dpy)]
}

/// ホイール 1 ノッチで次の zoom 値を計算する。
/// `bindings.next_wheel_zoom` (固定 zoom levels 上の階段) を呼ぶだけの薄い wrapper。
/// 端まで来ている / delta_y == 0 の場合は None を返し、caller は再 render を回避する。
pub(crate) fn next_wheel_zoom(
    evt: &WheelEvent,
    bindings: ViewControlBindings,
    current: f64,
) -> Option<f64> {
    bindings.next_wheel_zoom(current, evt.delta().strip_units().y)
}

/// Reference を Back / Front に分配する。両 canvas で同じ式が 4 回書かれていたので
/// ここに集約する。
pub(crate) fn partition_references(
    refs: &[SpriteReference],
) -> (Vec<SpriteReference>, Vec<SpriteReference>) {
    let back: Vec<_> = refs
        .iter()
        .filter(|r| r.placement == ReferencePlacement::Back)
        .cloned()
        .collect();
    let front: Vec<_> = refs
        .iter()
        .filter(|r| r.placement == ReferencePlacement::Front)
        .cloned()
        .collect();
    (back, front)
}

/// Body / Attack box の編集可能 overlay。両 canvas で共通の見た目（border + index バッジ +
/// 選択中の右下リサイズハンドル）を持つ。
///
/// ## 引数
/// - `position_style`: 親要素内での box の **配置とサイズ** を決める CSS。
///   `left/top/width/height` を全て含めて呼び出し側が指定する (zoom-multiplied CSS px)。
///   旧設計では width/height をこのコンポーネント内で付与していたが、wrapper の transform: scale
///   廃止に伴い zoom-multiplied 寸法も呼び出し側で計算する形に統一した。
/// - `on_start_move` / `on_start_resize`: mousedown 時に発火するハンドラ。canvas-specific
///   な DragKind を `dragging` signal に書き込む責務はこの handler が負う。primary click
///   チェックや stop_propagation も handler 側で行う（再生中ロックなど canvas ごとの
///   早期 return 条件を含めるため）。`on_start_resize` はどのハンドルを掴んだかを
///   `ResizeHandle` で受け取る (4 隅 + 4 辺の中点 = 計 8 種)。
#[component]
pub(crate) fn EditorBoxOverlay(
    target: SelectedBox,
    hitbox: HitBox,
    position_style: String,
    is_selected: bool,
    dimmed: bool,
    on_start_move: EventHandler<MouseEvent>,
    on_start_resize: EventHandler<(ResizeHandle, MouseEvent)>,
) -> Element {
    let _ = hitbox; // hitbox は呼び出し側が position_style に変換済み。残しておくのは型互換のため。
    let kind = target.kind();
    let color_class = kind.override_box_classes();
    let badge_class = kind.badge_classes();
    let label = format!("{}{}", kind.label_prefix(), target.index());
    let outline = if is_selected {
        "outline outline-1 outline-warning outline-offset-1"
    } else {
        ""
    };
    let opacity_class = if dimmed { "opacity-40" } else { "" };

    rsx! {
        div {
            // border-2 (2px) で zoom 非依存でも視認性を保つ。旧設計 (parent scale で 1px が
            // zoom 倍されて見える) からの代替で、zoom 1 時に 2 px、zoom 8 時にも 2 px 固定。
            class: "absolute border-2 pointer-events-auto cursor-move {color_class} {outline} {opacity_class}",
            style: "{position_style}",
            onmousedown: move |evt| on_start_move.call(evt),

            // index バッジ: 親 div の transform: scale を排した構成 (4K + 150% 対策、ui/README.md 参照)
            // なので zoom-inv での逆スケールは不要。font-size は固定 10px で zoom 非依存に表示する。
            span {
                class: "absolute leading-none px-1 py-0.5 font-mono pointer-events-none {badge_class}",
                style: "left: 0; top: 0; font-size: 10px;",
                "{label}"
            }

            if is_selected {
                ResizeHandles { on_start_resize }
            }
        }
    }
}

/// 4 隅 + 4 辺中点の 8 点リサイズハンドル。`is_selected` 時のみ親が呼び出す。
/// 各ハンドルは 8x8 px で、box の外側に -4px のオフセットで配置する (= box 枠と
/// 中心を合わせる)。辺中点は `calc(50% - 4px)` で box 中央に寄せる。
/// 親 wrapper に scale(zoom) は無くなった (4K + 150% 対策、ui/README.md 参照) ので、
/// ハンドルは zoom 非依存の固定 8x8 CSS px で表示される (旧 4x4 から拡大)。
#[component]
fn ResizeHandles(on_start_resize: EventHandler<(ResizeHandle, MouseEvent)>) -> Element {
    // (handle, position-style, cursor-class) の 8 点。座標は box 外側に -4px はみ出させる。
    let handles: [(ResizeHandle, &str, &str); 8] = [
        (
            ResizeHandle::TopLeft,
            "left: -4px; top: -4px;",
            "cursor-nwse-resize",
        ),
        (
            ResizeHandle::Top,
            "left: calc(50% - 4px); top: -4px;",
            "cursor-ns-resize",
        ),
        (
            ResizeHandle::TopRight,
            "right: -4px; top: -4px;",
            "cursor-nesw-resize",
        ),
        (
            ResizeHandle::Left,
            "left: -4px; top: calc(50% - 4px);",
            "cursor-ew-resize",
        ),
        (
            ResizeHandle::Right,
            "right: -4px; top: calc(50% - 4px);",
            "cursor-ew-resize",
        ),
        (
            ResizeHandle::BottomLeft,
            "left: -4px; bottom: -4px;",
            "cursor-nesw-resize",
        ),
        (
            ResizeHandle::Bottom,
            "left: calc(50% - 4px); bottom: -4px;",
            "cursor-ns-resize",
        ),
        (
            ResizeHandle::BottomRight,
            "right: -4px; bottom: -4px;",
            "cursor-nwse-resize",
        ),
    ];

    rsx! {
        for (handle, pos, cursor) in handles {
            div {
                key: "{handle:?}",
                class: "absolute bg-warning border border-base-100 {cursor}",
                style: "{pos} width: 8px; height: 8px;",
                onmousedown: move |evt| on_start_resize.call((handle, evt)),
            }
        }
    }
}
