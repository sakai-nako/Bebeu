use dioxus::prelude::*;

use crate::entities::level::Level;
use crate::entities::preference::use_preferences;
use crate::features::level::{
    DeleteLevelButton, EditBaseInline, LevelEditorActions, RenameLevelButton,
};
use crate::shared::use_history;
use crate::widgets::level::ui::{LevelCanvas, LevelInspector};

/// Level の編集画面 (Pattern D)。
///
/// 親 (LevelDetailPage) から渡された `level` を初期 baseline として draft / history を作り、
/// Canvas / Inspector / Actions に共有する。Save まで disk に書かない (NavigationGuard で
/// 離脱時の未保存ガードも有効)。base 画像 import / Rename / Delete は disk 操作なので Pattern A の
/// まま即時保存される (= LevelEditor の Save に乗らない独立した経路)。
#[component]
pub fn LevelDetail(level: Level) -> Element {
    let preferences = use_preferences();
    let history_capacity = preferences.peek().level_history_capacity as usize;

    let draft = use_signal(|| level.clone());
    let history = use_history(draft, history_capacity);

    rsx! {
        div { class: "flex flex-col gap-3 h-full",
            // Header (breadcrumb)
            div { class: "breadcrumbs text-sm",
                ul {
                    li { "levels" }
                    li { "{level.name}" }
                }
            }

            // Title + actions + Base + Save/Cancel
            div { class: "flex items-center gap-4 flex-wrap",
                h1 { class: "text-2xl font-bold", "{level.name}" }
                RenameLevelButton { level: level.clone() }
                DeleteLevelButton { name: level.name.clone() }
                div { class: "flex items-center gap-2 ml-4 pl-4 border-l border-base-300",
                    span { class: "text-sm font-semibold text-base-content/70", "Base:" }
                    EditBaseInline { level: level.clone() }
                }
                div { class: "ml-auto" }
                LevelEditorActions { original: level.clone(), draft, history }
            }

            p { class: "text-sm text-base-content/60",
                "緑の台形 = Area、青のカメラアイコン = カメラ開始位置、緑十字 = Player Spawn、黄破線 = Opponent Trigger。頂点・マーカーをドラッグ。ホイールで拡大縮小、middle button で pan。Ctrl+Z / Ctrl+Y で Undo / Redo、Ctrl+S で Save。"
            }

            // Canvas (左) + Inspector (右) の 2 カラム
            div { class: "flex-1 min-h-0 flex gap-3",
                div { class: "flex-1 min-w-0",
                    LevelCanvas { draft, history }
                }
                LevelInspector { draft, history }
            }
        }
    }
}
