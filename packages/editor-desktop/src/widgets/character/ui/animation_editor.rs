use dioxus::prelude::*;

use super::{
    AnimationCanvas, AnimationPropertyPanel, AnimationTimeline, CanvasVisibility, SpriteReference,
};
use crate::entities::character::{Animation, Character, SelectedBox, use_playback_provider};
use crate::entities::navigation_guard::use_navigation_guard;
use crate::entities::preference::use_preferences;
use crate::features::character::AnimationEditorActions;
use crate::shared::use_history;

#[component]
pub fn AnimationEditor(character: Character, animation: Animation) -> Element {
    let draft = use_signal(|| animation.clone());
    // 履歴上限は preferences から peek で取得（編集中の preference 変更で history を再生成しない）
    let preferences = use_preferences();
    let history_capacity = preferences.peek().animation_history_capacity as usize;
    let history = use_history(draft, history_capacity);
    let baseline = use_signal(|| animation.clone());
    let selected_frame_index = use_signal(|| 0_usize);
    let selected_layer_index = use_signal(|| None::<usize>);
    // Canvas と Panel が共有する Override box の選択状態。
    let selected_box = use_signal(|| None::<SelectedBox>);
    // Reference 表示はセッション内の表示設定。disk には書かないので draft とは独立に持つ。
    let references = use_signal(Vec::<SpriteReference>::new);
    // Canvas マーカー類の表示フラグ。同じくセッション内のみ。
    let visibility = use_signal(CanvasVisibility::default);
    // 再生状態。AnimationEditor 配下の子コンポーネント (Timeline / Canvas / Panel / Actions) が
    // use_playback() で取得する。Timeline がタイマースレッドの起動・停止を担当する。
    let _playback = use_playback_provider();
    let mut guard = use_navigation_guard();
    let nav = use_navigator();

    let character_url = format!("/characters/{}", character.name);

    rsx! {
        div { class: "flex flex-col gap-3 h-full",
            div { class: "flex items-center justify-between flex-wrap gap-2",
                div { class: "breadcrumbs text-sm",
                    ul {
                        li { "characters" }
                        li {
                            a {
                                class: "cursor-pointer",
                                onclick: move |_| guard.try_navigate(&nav, character_url.clone()),
                                "{character.name}"
                            }
                        }
                        li { "animations" }
                        li { "{animation.name}" }
                    }
                }
                AnimationEditorActions {
                    character: character.clone(),
                    original_animation: animation.clone(),
                    draft,
                    history,
                    baseline,
                    selected_frame_index,
                    selected_layer_index,
                }
            }

            // タイトル（Rename / Delete / Number 編集は CharacterDetail で行う）
            h1 { class: "text-2xl font-bold", "{animation.name}" }

            // T 字レイアウト: 左カラム (Canvas 上 / Timeline 下) + 右カラム (Property Panel 全高)。
            // Timeline は Canvas 幅と一致するため、Panel は縦に長く確保できる。
            div { class: "flex gap-3 flex-1 min-h-0",
                div { class: "flex-1 flex flex-col gap-3 min-w-0",
                    div { class: "flex-1 overflow-hidden bg-base-100 rounded-box",
                        AnimationCanvas {
                            character: character.clone(),
                            draft,
                            history,
                            selected_frame_index,
                            selected_layer_index,
                            selected_box,
                            references,
                            visibility,
                        }
                    }
                    AnimationTimeline {
                        character: character.clone(),
                        draft,
                        history,
                        selected_frame_index,
                        selected_layer_index,
                    }
                }
                div { class: "w-80 shrink-0",
                    AnimationPropertyPanel {
                        character: character.clone(),
                        draft,
                        history,
                        selected_frame_index,
                        selected_layer_index,
                        selected_box,
                        references,
                    }
                }
            }
        }
    }
}
