use dioxus::prelude::*;

use crate::entities::level::Level;
use crate::shared::UseHistory;

/// Level の `gravity_scale` (Option<f32>) を inline 編集するコンポーネント。
///
/// Pattern D: 親 (LevelEditor) の `draft` Signal を直接編集する。disk への保存は Save ボタン担当。
/// 空入力で「未設定 (= 1.0 相当)」に戻せるよう、入力欄が空白なら `None` をセットする。
#[component]
pub fn EditGravityScale(mut draft: Signal<Level>, mut history: UseHistory<Level>) -> Element {
    let mut editing = use_signal(|| false);
    let initial = draft.peek().clone();
    let mut draft_value = use_signal(|| format_scale(initial.gravity_scale));
    let mut error = use_signal(|| None::<String>);

    let on_edit = move |_| {
        let cur = draft.peek().clone();
        draft_value.set(format_scale(cur.gravity_scale));
        error.set(None);
        editing.set(true);
    };

    let on_apply = move |_| {
        let trimmed = draft_value();
        let trimmed = trimmed.trim();
        let new_scale: Option<f32> = if trimmed.is_empty() {
            None
        } else {
            match trimmed.parse::<f32>() {
                Ok(v) if v.is_finite() && v >= 0.0 => Some(v),
                Ok(_) => {
                    error.set(Some("0 以上の有限な数値を入力してください".into()));
                    return;
                }
                Err(_) => {
                    error.set(Some("0 以上の有限な数値か空欄を入力してください".into()));
                    return;
                }
            }
        };
        let cur = draft.peek().clone();
        if cur.gravity_scale != new_scale {
            history.record();
            draft.set(Level {
                gravity_scale: new_scale,
                ..cur
            });
        }
        editing.set(false);
        error.set(None);
    };

    let on_cancel = move |_| {
        editing.set(false);
        error.set(None);
    };

    let cur = draft.read();
    let display = format_scale(cur.gravity_scale);

    rsx! {
        if editing() {
            div { class: "flex flex-col gap-2",
                div { class: "flex items-center gap-1",
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        value: "{draft_value}",
                        step: "0.05",
                        min: "0",
                        placeholder: "1.0",
                        oninput: move |e| draft_value.set(e.value()),
                    }
                }
                p { class: "text-xs text-base-content/60",
                    "実効重力 = Character.physics.gravity × この値。空欄で 1.0 (= 通常重力)"
                }
                div { class: "flex gap-1",
                    button {
                        r#type: "button",
                        class: "btn btn-primary btn-xs",
                        onclick: on_apply,
                        "Apply"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-ghost btn-xs",
                        onclick: on_cancel,
                        "Cancel"
                    }
                }
                if let Some(message) = error() {
                    p { class: "text-error text-xs", "{message}" }
                }
            }
        } else {
            div { class: "flex items-center gap-2",
                span {
                    class: "font-mono text-sm",
                    title: "実効重力 = Character.physics.gravity × この値。未設定 (1.0) は通常重力",
                    "{display}"
                }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: on_edit,
                    title: "編集",
                    "✎"
                }
            }
        }
    }
}

fn format_scale(v: Option<f32>) -> String {
    match v {
        None => String::new(),
        Some(v) if v.fract() == 0.0 => format!("{v:.0}"),
        Some(v) => {
            let s = format!("{v:.3}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_scale_none_is_empty() {
        assert_eq!(format_scale(None), "");
    }

    #[test]
    fn format_scale_integer_no_decimal() {
        assert_eq!(format_scale(Some(1.0)), "1");
        assert_eq!(format_scale(Some(2.0)), "2");
    }

    #[test]
    fn format_scale_trims_trailing_zeros() {
        assert_eq!(format_scale(Some(0.5)), "0.5");
        assert_eq!(format_scale(Some(0.125)), "0.125");
    }
}
