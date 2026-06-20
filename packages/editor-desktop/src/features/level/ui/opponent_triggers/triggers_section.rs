use dioxus::prelude::*;

use super::TriggerRow;
use crate::entities::level::{Level, OpponentTrigger};
use crate::shared::UseHistory;

/// Opponent trigger 一覧 + 追加ボタン。`character_names` は Combobox の選択肢として渡される
/// (Character pool から呼び出し側で `CharacterRepository::list()` 経由で取得しておく)。
/// Pattern D: 親 (LevelEditor) の `draft` Signal を直接編集する。
#[component]
pub fn OpponentTriggersSection(
    mut draft: Signal<Level>,
    mut history: UseHistory<Level>,
    character_names: Vec<String>,
) -> Element {
    let on_add = move |_| {
        let cur = draft.peek().clone();
        let mut new_triggers = cur.opponent_triggers.clone();
        new_triggers.push(OpponentTrigger::default());
        history.record();
        draft.set(Level {
            opponent_triggers: new_triggers,
            ..cur
        });
    };

    let triggers_len = draft.read().opponent_triggers.len();

    rsx! {
        div { class: "space-y-2",
            if triggers_len == 0 {
                p { class: "text-sm text-base-content/60 italic",
                    "トリガーがまだありません。"
                }
            } else {
                for i in 0..triggers_len {
                    TriggerRow {
                        key: "trigger-{i}",
                        draft,
                        history,
                        index: i,
                        character_names: character_names.clone(),
                    }
                }
            }
            button {
                r#type: "button",
                class: "btn btn-primary btn-sm w-full",
                onclick: on_add,
                "+ Trigger を追加"
            }
        }
    }
}
