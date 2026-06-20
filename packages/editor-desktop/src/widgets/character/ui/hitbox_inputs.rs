//! HitBox の角座標を編集する number input の共通コンポーネント。
//!
//! Sprite 用 `BoxEditor` と Frame override 用 `BoxRow` の両方で 4 つの number input を
//! 並べる UI が登場する。コンテナのレイアウト (grid / inline row) と input サイズは
//! 異なるが、各 input の中身 (parse → onchange) はまったく同じパターンなので、ここに
//! 共通化する。

use dioxus::prelude::*;

use crate::shared::HitBoxCorner;

/// HitBox の 1 つの角座標 input。`onchange` でパース成功時のみ `on_change` を呼ぶ。
///
/// `class` は親側で指定する (例: BoxRow は `input-xs w-12`、BoxEditor は `input-sm w-full`)。
#[component]
pub(super) fn HitBoxCornerInput(
    corner: HitBoxCorner,
    value: i32,
    class: &'static str,
    on_change: EventHandler<i32>,
) -> Element {
    rsx! {
        input {
            r#type: "number",
            class,
            title: corner.title(),
            value: "{value}",
            onchange: move |evt| {
                if let Ok(v) = evt.value().trim().parse::<i32>() {
                    on_change.call(v);
                }
            },
        }
    }
}

/// HitBox.depth (`Option<u32>`) の number input。
///
/// - `None` のとき: input は空、placeholder にフォールバック値 (`character.depth`) を表示
/// - `Some(n)` のとき: 数値 n を表示
/// - 入力が空文字なら `None` に戻す、整数として parse できれば `Some(n)`、それ以外は無視
///
/// 値が変わらない (= `current` と同値) ときは on_change を呼ばない。履歴 / repository write を
/// 過剰に発火させないためで、呼び出し側で同値判定を別途しなくて良いようにここに寄せている。
#[component]
pub(super) fn HitBoxDepthInput(
    current: Option<u32>,
    fallback: u32,
    class: &'static str,
    on_change: EventHandler<Option<u32>>,
) -> Element {
    let display = current.map_or_else(String::new, |v| v.to_string());
    let placeholder = format!("{fallback} (inherit)");
    rsx! {
        input {
            r#type: "number",
            class,
            min: "0",
            title: "world Z 厚み。空欄 = Character.depth にフォールバック",
            value: "{display}",
            placeholder: "{placeholder}",
            onchange: move |evt| {
                let raw = evt.value();
                let trimmed = raw.trim();
                let next = if trimmed.is_empty() {
                    None
                } else if let Ok(v) = trimmed.parse::<u32>() {
                    Some(v)
                } else {
                    return;
                };
                if next == current {
                    return;
                }
                on_change.call(next);
            },
        }
    }
}
