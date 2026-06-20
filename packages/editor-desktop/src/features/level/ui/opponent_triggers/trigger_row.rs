use dioxus::prelude::*;

use super::DeleteTriggerButton;
use crate::entities::level::{Level, OpponentTrigger};
use crate::shared::UseHistory;

/// 1 個の Opponent trigger を表示 / 編集する行。Pattern D Inline (表示モード ↔ 編集モード)。
/// 編集モードでは Character select + trigger_x / spawn_x / spawn_y / spawn_z の number input 4 つ。
/// Apply で `draft` Signal を更新し、disk への保存は親 (LevelEditor) の Save ボタンが担う。
#[component]
pub fn TriggerRow(
    mut draft: Signal<Level>,
    mut history: UseHistory<Level>,
    index: usize,
    character_names: Vec<String>,
) -> Element {
    // 初期値は draft から取得 (Pattern D)
    let initial = draft
        .peek()
        .opponent_triggers
        .get(index)
        .cloned()
        .unwrap_or_default();

    // 新規追加直後の空 trigger (character_name 未設定) は自動で編集モードに入る。
    let auto_editing = initial.character_name.is_empty();
    let mut editing = use_signal(move || auto_editing);
    let mut draft_name = use_signal(|| initial.character_name.clone());
    let mut draft_tx = use_signal(|| initial.trigger_x.to_string());
    let mut draft_sx = use_signal(|| initial.spawn_x.to_string());
    let mut draft_sy = use_signal(|| initial.spawn_y.to_string());
    let mut draft_sz = use_signal(|| initial.spawn_z.to_string());
    let mut error = use_signal(|| None::<String>);

    let on_edit = move |_| {
        let cur = draft
            .peek()
            .opponent_triggers
            .get(index)
            .cloned()
            .unwrap_or_default();
        draft_name.set(cur.character_name.clone());
        draft_tx.set(cur.trigger_x.to_string());
        draft_sx.set(cur.spawn_x.to_string());
        draft_sy.set(cur.spawn_y.to_string());
        draft_sz.set(cur.spawn_z.to_string());
        error.set(None);
        editing.set(true);
    };

    let on_apply = move |_| {
        let Ok(tx) = draft_tx().trim().parse::<i32>() else {
            error.set(Some("trigger_x は整数で入力してください".into()));
            return;
        };
        let Ok(sx) = draft_sx().trim().parse::<i32>() else {
            error.set(Some("spawn_x は整数で入力してください".into()));
            return;
        };
        let Ok(sy) = draft_sy().trim().parse::<i32>() else {
            error.set(Some("spawn_y は整数で入力してください".into()));
            return;
        };
        let Ok(sz) = draft_sz().trim().parse::<i32>() else {
            error.set(Some("spawn_z は整数で入力してください".into()));
            return;
        };
        let new_trigger = OpponentTrigger {
            character_name: draft_name().trim().to_string(),
            trigger_x: tx,
            spawn_x: sx,
            spawn_y: sy,
            spawn_z: sz,
        };
        let cur = draft.peek().clone();
        let mut new_triggers = cur.opponent_triggers.clone();
        if let Some(slot) = new_triggers.get_mut(index) {
            if *slot == new_trigger {
                editing.set(false);
                error.set(None);
                return;
            }
            *slot = new_trigger;
        } else {
            error.set(Some("対象 trigger が見つかりません".into()));
            return;
        }
        history.record();
        draft.set(Level {
            opponent_triggers: new_triggers,
            ..cur
        });
        editing.set(false);
        error.set(None);
    };

    let on_cancel = move |_| {
        editing.set(false);
        error.set(None);
    };

    // 表示用は常に最新の draft 値から取得
    let current = draft
        .read()
        .opponent_triggers
        .get(index)
        .cloned()
        .unwrap_or_default();
    let pool_has_name =
        current.character_name.is_empty() || character_names.contains(&current.character_name);

    rsx! {
        div { class: "card bg-base-100 p-3 border border-base-300 space-y-2",
            div { class: "flex items-center justify-between",
                span { class: "badge badge-warning badge-sm", "#{index}" }
                if !editing() {
                    button {
                        r#type: "button",
                        class: "btn btn-ghost btn-xs",
                        onclick: on_edit,
                        title: "編集",
                        "✎"
                    }
                }
            }
            if editing() {
                div { class: "space-y-2",
                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend text-xs", "Character" }
                        select {
                            class: "select select-bordered select-sm w-full",
                            value: "{draft_name}",
                            oninput: move |e| draft_name.set(e.value()),
                            option { value: "", "(未選択)" }
                            for name in character_names.iter() {
                                option {
                                    value: "{name}",
                                    selected: name == &draft_name(),
                                    "{name}"
                                }
                            }
                        }
                    }
                    div { class: "grid grid-cols-2 gap-2",
                        label { class: "flex flex-col gap-0.5",
                            span { class: "text-xs text-base-content/60", "trigger_x" }
                            input {
                                r#type: "number",
                                class: "input input-bordered input-sm w-full",
                                value: "{draft_tx}",
                                step: "1",
                                oninput: move |e| draft_tx.set(e.value()),
                            }
                        }
                        label { class: "flex flex-col gap-0.5",
                            span { class: "text-xs text-base-content/60", "spawn_x" }
                            input {
                                r#type: "number",
                                class: "input input-bordered input-sm w-full",
                                value: "{draft_sx}",
                                step: "1",
                                oninput: move |e| draft_sx.set(e.value()),
                            }
                        }
                        label { class: "flex flex-col gap-0.5",
                            span { class: "text-xs text-base-content/60", "spawn_y" }
                            input {
                                r#type: "number",
                                class: "input input-bordered input-sm w-full",
                                value: "{draft_sy}",
                                step: "1",
                                oninput: move |e| draft_sy.set(e.value()),
                            }
                        }
                        label { class: "flex flex-col gap-0.5",
                            span { class: "text-xs text-base-content/60", "spawn_z" }
                            input {
                                r#type: "number",
                                class: "input input-bordered input-sm w-full",
                                value: "{draft_sz}",
                                step: "1",
                                oninput: move |e| draft_sz.set(e.value()),
                            }
                        }
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
                        DeleteTriggerButton { draft, history, index }
                    }
                    if let Some(message) = error() {
                        p { class: "text-error text-xs", "{message}" }
                    }
                }
            } else {
                div { class: "text-sm space-y-0.5",
                    if current.character_name.is_empty() {
                        div { class: "font-semibold text-warning", "(未設定)" }
                    } else {
                        div { class: "font-semibold", "{current.character_name}" }
                    }
                    div { class: "font-mono text-xs text-base-content/70",
                        "trigger_x = {current.trigger_x}"
                    }
                    div { class: "font-mono text-xs text-base-content/70",
                        "spawn = ({current.spawn_x}, {current.spawn_y}, {current.spawn_z})"
                    }
                    if !pool_has_name {
                        p { class: "text-warning text-xs",
                            "⚠ Character pool に '{current.character_name}' が見つかりません"
                        }
                    }
                }
            }
        }
    }
}
