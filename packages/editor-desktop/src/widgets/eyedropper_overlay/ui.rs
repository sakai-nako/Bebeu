//! 透明フルスクリーン overlay window で画面の色を拾う (Plan: atomic-questing-wozniak)。
//!
//! 親 popover が `spawn_eyedropper(capture_sig)` で背景キャプチャを開始し、結果を受け取って
//! `new_window` でこの overlay component の VirtualDom を起動する。結果の受け渡しは
//! `Arc<Mutex<Option<...>>>` で行う (`Signal` を別 VirtualDom 間で共有すると owning scope
//! が drop されたとき危険、と Dioxus が警告するため)。overlay は `use_drop` で「明示的な
//! クリック / ESC で結果が設定されていなければ Cancelled を sink に入れる」保険を持つ。
use std::fmt;
use std::sync::{Arc, Mutex};

use dioxus::desktop::tao::dpi::{PhysicalPosition, PhysicalSize};
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use dioxus_signals::SyncStorage;

use crate::entities::project::HexColor;
use crate::shared::screen_capture::{
    CaptureError, CaptureResult, capture_virtual_screen, pixel_at,
};

/// overlay と親 popover が結果を受け渡すための共有 sink。
pub type PickSink = Arc<Mutex<Option<Result<HexColor, PickError>>>>;

#[derive(Debug, Clone)]
pub enum PickError {
    Cancelled,
    Capture(String),
    OutOfRange,
}

impl fmt::Display for PickError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "色取得をキャンセルしました"),
            Self::Capture(m) => write!(f, "画面キャプチャに失敗: {m}"),
            Self::OutOfRange => write!(f, "クリック位置が画面外です"),
        }
    }
}

impl std::error::Error for PickError {}

/// 別スレッドで virtual screen を撮影し、結果を `capture_sig` に書き戻す。
/// ADR-0002: editor は async を使わないため `std::thread::spawn` を使う。
pub fn spawn_eyedropper(
    capture_sig: Signal<Option<Result<CaptureResult, CaptureError>>, SyncStorage>,
) {
    let mut capture_sig = capture_sig;
    std::thread::spawn(move || {
        let r = capture_virtual_screen();
        capture_sig.set(Some(r));
    });
}

/// overlay window 用の dioxus Config を組み立てる。virtual screen 全域に物理 px 単位で
/// 配置し、装飾なし・透明・always-on-top にする。
#[must_use]
pub fn build_overlay_config(capture: &CaptureResult) -> Config {
    let (ox, oy) = capture.layout.origin;
    let (w, h) = capture.layout.size;
    let wb = WindowBuilder::new()
        .with_transparent(true)
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top(true)
        .with_position(PhysicalPosition::new(ox, oy))
        .with_inner_size(PhysicalSize::new(w, h))
        .with_title("Eyedropper");
    Config::new()
        .with_window(wb)
        .with_background_color((0, 0, 0, 0))
        .with_menu(None)
        .with_disable_context_menu(true)
}

#[derive(Clone, Props)]
pub struct OverlayProps {
    pub sink: PickSink,
    pub capture: CaptureResult,
}

impl PartialEq for OverlayProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.sink, &other.sink) && self.capture == other.capture
    }
}

// 虫眼鏡パラメータ
const ZOOM: f64 = 12.0;
const LENS_SIZE: f64 = 160.0;

#[component]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn EyedropperOverlay(props: OverlayProps) -> Element {
    // overlay 内のカーソル位置を CSS pixel で保持し、scale_factor で physical px に変換する。
    let mut cursor_css = use_signal(|| (0.0_f64, 0.0_f64));
    // tao window の scale_factor (overlay window 単位、複数モニタ間の差はカーソル位置のモニタ依存)。
    let scale_factor = dioxus::desktop::window().scale_factor();

    let sink = props.sink.clone();
    let capture = props.capture.clone();
    let png_url: String = capture.png_data_url.as_ref().to_string();
    let (vw, vh) = capture.layout.size;

    let capture_for_click = capture.clone();
    let sink_click = sink.clone();
    let on_click = move |evt: MouseEvent| {
        let c = evt.client_coordinates();
        let cx = (c.x * scale_factor).round() as i32 + capture_for_click.layout.origin.0;
        let cy = (c.y * scale_factor).round() as i32 + capture_for_click.layout.origin.1;
        let next = pixel_at(&capture_for_click, cx, cy).ok_or(PickError::OutOfRange);
        *sink_click.lock().expect("eyedropper sink mutex poisoned") = Some(next);
        dioxus::desktop::window().close();
    };

    let on_move = move |evt: MouseEvent| {
        let c = evt.client_coordinates();
        cursor_css.set((c.x, c.y));
    };

    let sink_key = sink.clone();
    let on_keydown = move |evt: KeyboardEvent| {
        if evt.key() == Key::Escape {
            *sink_key.lock().expect("eyedropper sink mutex poisoned") =
                Some(Err(PickError::Cancelled));
            dioxus::desktop::window().close();
        }
    };

    // overlay が「明示的な確定/中止なしに」閉じられた場合 (ウィンドウ X クリック等) の保険。
    // 親 popover の polling thread が永久ループしないよう、必ず sink に値が入る状態にする。
    let sink_drop = sink.clone();
    use_drop(move || {
        let mut g = sink_drop.lock().expect("eyedropper sink mutex poisoned");
        if g.is_none() {
            *g = Some(Err(PickError::Cancelled));
        }
    });

    // overlay の CSS pixel サイズ (style 用): physical / scale_factor
    let css_w = f64::from(vw) / scale_factor;
    let css_h = f64::from(vh) / scale_factor;

    let (mx_css, my_css) = cursor_css();

    let lens_offset = 24.0_f64;
    let lens_left = mx_css + lens_offset;
    let lens_top = my_css + lens_offset;

    // 虫眼鏡内の画像の transform: scale 後にカーソル位置のピクセルが中心に来るようにする。
    let img_tx = -(mx_css * ZOOM - LENS_SIZE / 2.0);
    let img_ty = -(my_css * ZOOM - LENS_SIZE / 2.0);

    let hint_style = "position: absolute; top: 12px; right: 12px; background: rgba(0,0,0,0.7); color: white; \
         padding: 6px 10px; border-radius: 6px; font-size: 12px; font-family: sans-serif;";

    rsx! {
        div {
            id: "eyedropper-root",
            tabindex: 0,
            autofocus: true,
            onkeydown: on_keydown,
            onmousemove: on_move,
            onclick: on_click,
            style: "position: fixed; inset: 0; cursor: crosshair; background: rgba(0,0,0,0); overflow: hidden;",

            // 背景: virtual screen 全域のスクリーンショット
            img {
                src: "{png_url}",
                draggable: "false",
                style: "position: absolute; left: 0; top: 0; width: {css_w}px; height: {css_h}px; image-rendering: pixelated; user-select: none; pointer-events: none;",
            }

            // 虫眼鏡 (カーソル追従、12x 拡大)
            div {
                style: "position: absolute; left: {lens_left}px; top: {lens_top}px; width: {LENS_SIZE}px; height: {LENS_SIZE}px; border-radius: 50%; overflow: hidden; pointer-events: none; box-shadow: 0 0 12px rgba(0,0,0,0.8); border: 2px solid white;",
                img {
                    src: "{png_url}",
                    draggable: "false",
                    style: "position: absolute; left: 0; top: 0; width: {css_w}px; height: {css_h}px; image-rendering: pixelated; transform-origin: 0 0; transform: translate({img_tx}px, {img_ty}px) scale({ZOOM}); user-select: none;",
                }
                // 中心 1px (拡大後 ZOOM px) を白枠で強調
                div {
                    style: "position: absolute; left: {LENS_SIZE / 2.0 - ZOOM / 2.0}px; top: {LENS_SIZE / 2.0 - ZOOM / 2.0}px; width: {ZOOM}px; height: {ZOOM}px; border: 1px solid white; box-sizing: border-box; pointer-events: none; mix-blend-mode: difference;",
                }
            }

            // ヒント表示
            div { style: "{hint_style}",
                "クリックで確定 / ESC で中止"
            }
        }
    }
}
