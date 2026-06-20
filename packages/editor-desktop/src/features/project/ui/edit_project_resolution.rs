use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::project::{Project, ProjectRepository, Resolution};

/// 妥当な範囲。1 未満は不可、極端に大きい値も誤入力として弾く。
const MIN_DIMENSION: u32 = 1;
const MAX_DIMENSION: u32 = 7680;

/// 指定された Project Signal の論理解像度を編集する。
///
/// 数値入力は String で持って onchange で確定するパターン。disk 保存に成功してから
/// project Signal を更新する (memory と disk の乖離を避ける)。
#[component]
pub fn EditProjectResolution(project: Signal<Project>) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();
    let mut error = use_signal(|| None::<String>);

    let current = project.read().resolution;
    let mut width_input = use_signal(|| current.width.to_string());
    let mut height_input = use_signal(|| current.height.to_string());

    // 外部から project が変わったら入力欄も追従させる
    use_effect(move || {
        let r = project.read().resolution;
        width_input.set(r.width.to_string());
        height_input.set(r.height.to_string());
    });

    let on_width_change = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            commit(&repo, project, &mut error, Axis::Width, &evt.value());
        }
    };
    let on_height_change = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            commit(&repo, project, &mut error, Axis::Height, &evt.value());
        }
    };

    rsx! {
        fieldset { class: "fieldset",
            legend { class: "fieldset-legend", "論理解像度" }
            div { class: "flex items-center gap-2",
                input {
                    r#type: "number",
                    class: "input input-bordered w-28",
                    min: i64::from(MIN_DIMENSION),
                    max: i64::from(MAX_DIMENSION),
                    value: "{width_input}",
                    oninput: move |evt| width_input.set(evt.value()),
                    onchange: on_width_change,
                }
                span { class: "text-base-content/60", "×" }
                input {
                    r#type: "number",
                    class: "input input-bordered w-28",
                    min: i64::from(MIN_DIMENSION),
                    max: i64::from(MAX_DIMENSION),
                    value: "{height_input}",
                    oninput: move |evt| height_input.set(evt.value()),
                    onchange: on_height_change,
                }
                span { class: "text-base-content/60 text-sm", "px" }
            }
            p { class: "text-xs text-base-content/60 mt-1",
                "engine が描画する論理解像度。表示ウィンドウサイズと比率が異なる場合は engine 側でレターボックス調整される。変更は engine の次回起動時から反映。"
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Axis {
    Width,
    Height,
}

/// 入力値を validation し、新しい Resolution を計算する純関数。
fn next_resolution(current: Resolution, axis: Axis, parsed: u32) -> Resolution {
    match axis {
        Axis::Width => Resolution {
            width: parsed,
            height: current.height,
        },
        Axis::Height => Resolution {
            width: current.width,
            height: parsed,
        },
    }
}

fn commit(
    repo: &Arc<dyn ProjectRepository>,
    mut project: Signal<Project>,
    error: &mut Signal<Option<String>>,
    axis: Axis,
    raw: &str,
) {
    let Ok(parsed) = raw.trim().parse::<u32>() else {
        error.set(Some(format!(
            "{MIN_DIMENSION} 〜 {MAX_DIMENSION} の整数で入力してください"
        )));
        return;
    };
    if !(MIN_DIMENSION..=MAX_DIMENSION).contains(&parsed) {
        error.set(Some(format!(
            "{MIN_DIMENSION} 〜 {MAX_DIMENSION} の整数で入力してください"
        )));
        return;
    }
    let mut next = project.peek().clone();
    next.resolution = next_resolution(next.resolution, axis, parsed);
    match repo.update(&next) {
        Ok(()) => {
            project.set(next);
            error.set(None);
        }
        Err(e) => error.set(Some(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_resolution_updates_width_only() {
        let r = Resolution {
            width: 640,
            height: 360,
        };
        let next = next_resolution(r, Axis::Width, 1280);
        assert_eq!(next.width, 1280);
        assert_eq!(next.height, 360);
    }

    #[test]
    fn next_resolution_updates_height_only() {
        let r = Resolution {
            width: 640,
            height: 360,
        };
        let next = next_resolution(r, Axis::Height, 720);
        assert_eq!(next.width, 640);
        assert_eq!(next.height, 720);
    }
}
