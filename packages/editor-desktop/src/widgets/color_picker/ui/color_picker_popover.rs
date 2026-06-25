//! RGBA 同居 popover カラーピッカー (Plan: atomic-questing-wozniak)。
//!
//! HTML `<input type="color">` の制約 (RGB のみ、マルチディスプレイでスポイト不可) を回避する
//! ための自前実装。daisyUI dropdown ベースで `:focus-within` 開閉。
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dioxus::prelude::*;

use dioxus::prelude::VirtualDom;

use crate::entities::project::HexColor;
use crate::shared::color_hsv::{hsv_to_rgb, rgb_to_hsv};
use crate::shared::screen_capture::{CaptureError, CaptureResult};
use crate::widgets::eyedropper_overlay::{
    EyedropperOverlay, OverlayProps, PickError, PickSink, build_overlay_config, spawn_eyedropper,
};

#[component]
#[allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn ColorPickerPopover(
    value: HexColor,
    on_change: EventHandler<HexColor>,
    #[props(default)] label: Option<&'static str>,
) -> Element {
    // HSV ローカル draft。grey 軸 (S=0) でも hue/value を独立に動かせる UX のため、
    // RGB → HSV にまとめず HSV のまま保持する。
    let mut hsv = use_signal(|| rgb_to_hsv(value.r, value.g, value.b));
    let mut alpha = use_signal(|| value.a);
    let mut hex_text = use_signal(|| value.to_hex_string());
    let mut hex_valid = use_signal(|| true);

    // 外部 value 更新 → ローカル状態に反映。HSV→RGB round-trip 後に値が一致しているなら no-op
    // (slider 操作 → on_change → 親 → ここに戻る無限ループを防ぐ)。
    use_effect(move || {
        let v = value;
        let cur_rgb = hsv_to_rgb(*hsv.peek());
        let cur_alpha = *alpha.peek();
        if cur_rgb != (v.r, v.g, v.b) || cur_alpha != v.a {
            hsv.set(rgb_to_hsv(v.r, v.g, v.b));
            alpha.set(v.a);
            hex_text.set(v.to_hex_string());
            hex_valid.set(true);
        }
    });

    let cur_h = hsv().h.round() as i32;
    let cur_s = (hsv().s * 100.0).round() as i32;
    let cur_v = (hsv().v * 100.0).round() as i32;
    let cur_alpha_u8 = alpha();
    let (cur_r, cur_g, cur_b) = hsv_to_rgb(hsv());

    let on_hue = move |evt: Event<FormData>| {
        if let Ok(h) = evt.value().parse::<f32>() {
            let mut cur = hsv();
            cur.h = h;
            hsv.set(cur);
            let (r, g, b) = hsv_to_rgb(cur);
            let next = HexColor {
                r,
                g,
                b,
                a: alpha(),
            };
            hex_text.set(next.to_hex_string());
            hex_valid.set(true);
            on_change.call(next);
        }
    };
    let on_sat = move |evt: Event<FormData>| {
        if let Ok(s) = evt.value().parse::<f32>() {
            let mut cur = hsv();
            cur.s = (s / 100.0).clamp(0.0, 1.0);
            hsv.set(cur);
            let (r, g, b) = hsv_to_rgb(cur);
            let next = HexColor {
                r,
                g,
                b,
                a: alpha(),
            };
            hex_text.set(next.to_hex_string());
            hex_valid.set(true);
            on_change.call(next);
        }
    };
    let on_val = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().parse::<f32>() {
            let mut cur = hsv();
            cur.v = (v / 100.0).clamp(0.0, 1.0);
            hsv.set(cur);
            let (r, g, b) = hsv_to_rgb(cur);
            let next = HexColor {
                r,
                g,
                b,
                a: alpha(),
            };
            hex_text.set(next.to_hex_string());
            hex_valid.set(true);
            on_change.call(next);
        }
    };
    let on_alpha = move |evt: Event<FormData>| {
        if let Ok(a) = evt.value().parse::<u32>() {
            let a = a.min(255) as u8;
            alpha.set(a);
            let (r, g, b) = hsv_to_rgb(hsv());
            let next = HexColor { r, g, b, a };
            hex_text.set(next.to_hex_string());
            hex_valid.set(true);
            on_change.call(next);
        }
    };
    // R / G / B を 0-255 で直接編集する。HSV draft は新しい RGB から rederive する。
    let on_r = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().parse::<u32>() {
            let new_r = v.min(255) as u8;
            let (_, g, b) = hsv_to_rgb(hsv());
            hsv.set(rgb_to_hsv(new_r, g, b));
            let next = HexColor {
                r: new_r,
                g,
                b,
                a: alpha(),
            };
            hex_text.set(next.to_hex_string());
            hex_valid.set(true);
            on_change.call(next);
        }
    };
    let on_g = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().parse::<u32>() {
            let new_g = v.min(255) as u8;
            let (r, _, b) = hsv_to_rgb(hsv());
            hsv.set(rgb_to_hsv(r, new_g, b));
            let next = HexColor {
                r,
                g: new_g,
                b,
                a: alpha(),
            };
            hex_text.set(next.to_hex_string());
            hex_valid.set(true);
            on_change.call(next);
        }
    };
    let on_b = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().parse::<u32>() {
            let new_b = v.min(255) as u8;
            let (r, g, _) = hsv_to_rgb(hsv());
            hsv.set(rgb_to_hsv(r, g, new_b));
            let next = HexColor {
                r,
                g,
                b: new_b,
                a: alpha(),
            };
            hex_text.set(next.to_hex_string());
            hex_valid.set(true);
            on_change.call(next);
        }
    };
    let on_hex_input = move |evt: Event<FormData>| {
        let raw = evt.value();
        hex_text.set(raw.clone());
        match HexColor::parse(&raw) {
            Ok(c) => {
                hex_valid.set(true);
                hsv.set(rgb_to_hsv(c.r, c.g, c.b));
                alpha.set(c.a);
                on_change.call(c);
            }
            Err(_) => hex_valid.set(false),
        }
    };
    let on_hex_blur = move |_| {
        if !hex_valid() {
            let (r, g, b) = hsv_to_rgb(hsv());
            let cur = HexColor {
                r,
                g,
                b,
                a: alpha(),
            };
            hex_text.set(cur.to_hex_string());
            hex_valid.set(true);
        }
    };

    // スポイト: ① 別スレッドで virtual screen を撮影 → capture_sig に届く。
    //          ② capture を受けた use_effect が main thread で overlay window を spawn し、
    //             結果監視 thread も起動する (overlay 側からは Arc<Mutex> 経由で sink に書く)。
    //          ③ 結果監視 thread が sink に書かれた値を pick_result Signal に転記。
    //          ④ use_effect が pick_result を見て popover state に反映。
    // (ADR-0002: async fn / .await は使わない。`new_window` の戻り PendingDesktopContext は drop してよい)
    // sink は popover の hook で生成して所有することで、別 VirtualDom (overlay) に Signal を直接
    // 渡すことによる owning scope の混乱 (Dioxus の WARN) を回避する。
    let pick_sink: PickSink = use_hook(|| Arc::new(Mutex::new(None)));
    let capture_sig = use_signal_sync(|| None::<Result<CaptureResult, CaptureError>>);
    let mut pick_result = use_signal_sync(|| None::<Result<HexColor, PickError>>);
    let mut picking = use_signal(|| false);
    let mut open = use_signal(|| false);
    let on_pick = {
        let pick_sink = pick_sink.clone();
        move |_| {
            if picking() {
                return;
            }
            picking.set(true);
            open.set(false);
            *pick_sink.lock().expect("eyedropper sink mutex poisoned") = None;
            spawn_eyedropper(capture_sig);
        }
    };
    // capture 完了 → overlay window と結果監視 thread を起動
    use_effect({
        let pick_sink = pick_sink.clone();
        move || {
            let mut capture_sig = capture_sig;
            let Some(result) = capture_sig() else {
                return;
            };
            capture_sig.set(None);
            match result {
                Ok(capture) => {
                    let dom = VirtualDom::new_with_props(
                        EyedropperOverlay,
                        OverlayProps {
                            sink: pick_sink.clone(),
                            capture: capture.clone(),
                        },
                    );
                    let cfg = build_overlay_config(&capture);
                    let _ = dioxus::desktop::window().new_window(dom, cfg);

                    // overlay 側が sink に値を書くまで polling (overlay の use_drop が必ず
                    // Cancelled を入れるので無限ループにはならない)。
                    let sink_poll = pick_sink.clone();
                    let mut pick_result_sig = pick_result;
                    std::thread::spawn(move || {
                        loop {
                            std::thread::sleep(Duration::from_millis(50));
                            let mut g = sink_poll.lock().expect("eyedropper sink mutex poisoned");
                            if let Some(r) = g.take() {
                                pick_result_sig.set(Some(r));
                                return;
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("eyedropper capture failed: {e}");
                    pick_result.set(Some(Err(PickError::Capture(e.to_string()))));
                }
            }
        }
    });
    // pick 結果が届いたら state を更新
    use_effect(move || {
        let Some(result) = pick_result() else {
            return;
        };
        pick_result.set(None);
        picking.set(false);
        if let Ok(c) = result {
            hsv.set(rgb_to_hsv(c.r, c.g, c.b));
            alpha.set(c.a);
            hex_text.set(c.to_hex_string());
            hex_valid.set(true);
            on_change.call(c);
        }
    });
    let is_picking = picking();

    // popover の開閉は daisyUI dropdown の `:focus-within` ベースだと、Hex input をクリックした
    // 瞬間の focus 遷移で別 popover の判定がチラついて誤発火する。signal で明示制御し、
    // 背景に透明 backdrop を置いて外側クリックで閉じる方式に統一する (`open` は on_pick より
    // 前で宣言済み)。

    let hex_class = if hex_valid() {
        "input input-sm input-bordered w-full font-mono"
    } else {
        "input input-sm input-bordered input-error w-full font-mono"
    };
    // hue track の見た目を rainbow gradient に。
    let hue_track_bg = "linear-gradient(to right, #f00 0%, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00 100%)";

    rsx! {
        div { class: "relative inline-block",
            // trigger: 現在色の swatch ボタン (本物の <button> でクリック領域を明確に)
            button {
                r#type: "button",
                class: "btn btn-sm border border-base-300 p-0 overflow-hidden",
                onclick: move |_| open.set(!open()),
                if let Some(l) = label {
                    span { class: "text-xs px-1", "{l}" }
                }
                div {
                    class: "h-6 w-12 checkerboard-bg",
                    div {
                        class: "h-full w-full",
                        style: "background-color: rgba({cur_r}, {cur_g}, {cur_b}, {f32::from(cur_alpha_u8) / 255.0:.3});",
                    }
                }
            }
            if open() {
                // backdrop: popover 外クリックで close
                div {
                    style: "position: fixed; inset: 0; z-index: 40;",
                    onclick: move |_| open.set(false),
                }
            }
            if open() {
            div {
                style: "position: absolute; left: 0; top: 100%; margin-top: 0.25rem; z-index: 50;",
                class: "bg-base-200 rounded-box shadow p-3 w-72 flex flex-col gap-2",
                div { class: "form-control",
                    span { class: "label-text text-xs", "Hue {cur_h}°" }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "360",
                        step: "1",
                        value: "{cur_h}",
                        class: "range range-xs",
                        style: "background: {hue_track_bg};",
                        oninput: on_hue,
                    }
                }
                div { class: "form-control",
                    span { class: "label-text text-xs", "Saturation {cur_s}%" }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "100",
                        step: "1",
                        value: "{cur_s}",
                        class: "range range-xs",
                        oninput: on_sat,
                    }
                }
                div { class: "form-control",
                    span { class: "label-text text-xs", "Value {cur_v}%" }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "100",
                        step: "1",
                        value: "{cur_v}",
                        class: "range range-xs",
                        oninput: on_val,
                    }
                }
                div { class: "form-control",
                    span { class: "label-text text-xs", "R {cur_r}" }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "255",
                        step: "1",
                        value: "{cur_r}",
                        class: "range range-xs",
                        oninput: on_r,
                    }
                }
                div { class: "form-control",
                    span { class: "label-text text-xs", "G {cur_g}" }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "255",
                        step: "1",
                        value: "{cur_g}",
                        class: "range range-xs",
                        oninput: on_g,
                    }
                }
                div { class: "form-control",
                    span { class: "label-text text-xs", "B {cur_b}" }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "255",
                        step: "1",
                        value: "{cur_b}",
                        class: "range range-xs",
                        oninput: on_b,
                    }
                }
                div { class: "form-control",
                    span { class: "label-text text-xs", "A {cur_alpha_u8}" }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "255",
                        step: "1",
                        value: "{cur_alpha_u8}",
                        class: "range range-xs",
                        oninput: on_alpha,
                    }
                }
                div { class: "flex items-end gap-2",
                    div { class: "form-control flex-1",
                        span { class: "label-text text-xs", "Hex" }
                        input {
                            r#type: "text",
                            class: "{hex_class}",
                            value: "{hex_text()}",
                            oninput: on_hex_input,
                            onblur: on_hex_blur,
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-sm btn-square",
                        disabled: is_picking,
                        title: if is_picking { "画面のどこかをクリックしてください (ESC で中止)" } else { "画面から色を取る" },
                        onclick: on_pick,
                        if is_picking { "…" } else { "◎" }
                    }
                }
            }
            }
        }
    }
}
