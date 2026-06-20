use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::CharacterRepository;
use crate::entities::level::Level;
use crate::features::level::{
    EditCameraStart, EditGravityScale, EditPlayerRespawnY, EditPlayerSpawn, OpponentTriggersSection,
};
use crate::shared::UseHistory;

/// LevelEditor の右サイドバー。Camera 開始位置 / Player Spawn / Opponent Triggers を編集する。
///
/// Pattern D: 親 (LevelEditor) の draft / history を受け取り、子の各 form も同じ Signal を共有する。
/// Character pool は `CharacterRepository::list()` で一度だけロードして、Opponent trigger 編集行の
/// Character 選択 Combobox に渡す。
#[component]
pub fn LevelInspector(draft: Signal<Level>, history: UseHistory<Level>) -> Element {
    let char_repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut character_names = use_signal(Vec::<String>::new);
    use_effect(move || match char_repo.list() {
        Ok(list) => character_names.set(list.into_iter().map(|c| c.name).collect()),
        Err(e) => tracing::warn!("Character list の取得に失敗: {}", e),
    });

    rsx! {
        aside { class: "w-80 shrink-0 bg-base-200 border-l border-base-300 overflow-y-auto p-4 space-y-4",
            section {
                h2 { class: "text-sm font-semibold uppercase text-base-content/60 mb-2",
                    "Camera"
                }
                div { class: "card bg-base-100 p-3",
                    EditCameraStart { draft, history }
                }
            }
            section {
                h2 { class: "text-sm font-semibold uppercase text-base-content/60 mb-2",
                    "Player Spawn"
                }
                div { class: "card bg-base-100 p-3 space-y-2",
                    EditPlayerSpawn { draft, history }
                    div { class: "divider my-1" }
                    EditPlayerRespawnY { draft, history }
                }
            }
            section {
                h2 { class: "text-sm font-semibold uppercase text-base-content/60 mb-2",
                    "Physics"
                }
                div { class: "card bg-base-100 p-3 space-y-1",
                    div { class: "text-xs text-base-content/60", "Gravity Scale" }
                    EditGravityScale { draft, history }
                }
            }
            section {
                h2 { class: "text-sm font-semibold uppercase text-base-content/60 mb-2",
                    "Opponent Triggers"
                }
                OpponentTriggersSection {
                    draft,
                    history,
                    character_names: character_names(),
                }
            }
        }
    }
}
