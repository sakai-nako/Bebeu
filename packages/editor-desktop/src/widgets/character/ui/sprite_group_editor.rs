use std::sync::Arc;

use dioxus::prelude::*;

use super::canvas_visibility::CanvasVisibility;
use super::sprite_canvas::{DragState, SpriteCanvas};
use super::sprite_editor_sidebar::SpriteEditorSidebar;
use super::sprite_property_panel::SpritePropertyPanel;
use super::sprite_reference::SpriteReference;
use crate::entities::character::{Character, CharacterRepository, SelectedBox, SpriteGroup};
use crate::entities::navigation_guard::use_navigation_guard;
use crate::entities::preference::use_preferences;
use crate::features::character::SpriteGroupEditorActions;
use crate::shared::{SpriteDiskOps, use_history};

#[component]
pub fn SpriteGroupEditor(character: Character, sprite_group: SpriteGroup) -> Element {
    let draft = use_signal(|| sprite_group.clone());
    // 履歴上限は preferences から取得する。peek() を使うのは、編集中に preferences を変えても
    // 開いている editor の history は再生成されない (use_signal は初期化関数が初回のみ実行)
    // ため、再 subscribe する意味がないから。次回 editor を開いたタイミングで反映される。
    let preferences = use_preferences();
    let history_capacity = preferences.peek().sprite_group_history_capacity as usize;
    let history = use_history(draft, history_capacity);
    let selected_sprite_index = use_signal(|| 0_usize);
    let selected_box = use_signal(|| None::<SelectedBox>);
    let dragging = use_signal(|| None::<DragState>);
    let disk_ops = use_signal(SpriteDiskOps::default);
    // 画像 URL のキャッシュバスタは app_root で provider 済み。bump は ReimportSpritesScaledButton や
    // import / replace の handler 内で行われる。
    // Reference 表示はセッション内の表示設定。disk には書かないので draft とは独立に持つ。
    let references = use_signal(Vec::<SpriteReference>::new);
    // Canvas マーカー類の表示フラグ。同じく session 内のみで永続化しない。
    let visibility = use_signal(CanvasVisibility::default);
    let mut guard = use_navigation_guard();
    let nav = use_navigator();

    // breadcrumb のリンク先 URL（character ページに戻る用）
    let character_url = format!("/characters/{}", character.name);

    // unmount 時に未コミットの画像 disk 操作を rollback する。
    // 「Cancel して Editor を抜けた」「ナビ起点で破棄して移動」「Save 後の再 mount」全てで走る。
    // Save 成功時は disk_ops が空 (SpriteGroupEditorActions が clear する) のでこの処理は no-op。
    //
    // - pending_imports: disk 上にコピーされたが yml 未登録 → delete で巻き戻し
    // - pending_overwrites: 上書き import で .bak がある → restore で旧画像に戻す
    {
        let repo = use_context::<Arc<dyn CharacterRepository>>();
        let character_name = character.name.clone();
        let sprite_group_name = sprite_group.name.clone();
        let disk_ops_handle = disk_ops;
        use_drop(move || {
            let ops = disk_ops_handle.peek().clone();
            for basename in &ops.pending_imports {
                let _ = repo.delete_sprite_image(&character_name, &sprite_group_name, basename);
            }
            for basename in &ops.pending_overwrites {
                let _ =
                    repo.restore_sprite_image_backup(&character_name, &sprite_group_name, basename);
            }
        });
    }

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
                        li { "sprite-groups" }
                        li { "{sprite_group.name}" }
                    }
                }
                SpriteGroupEditorActions {
                    character: character.clone(),
                    original_group: sprite_group.clone(),
                    draft,
                    history,
                    selected_sprite_index,
                    selected_box,
                    disk_ops,
                }
            }

            // タイトル（Rename / Delete / Number 編集は CharacterDetail で行う）
            h1 { class: "text-2xl font-bold", "{sprite_group.name}" }

            div { class: "flex gap-3 flex-1 min-h-0",
                div { class: "w-56 shrink-0",
                    SpriteEditorSidebar {
                        character: character.clone(),
                        sprite_group: sprite_group.clone(),
                        draft,
                        history,
                        selected_sprite_index,
                        selected_box,
                        disk_ops,
                    }
                }
                div { class: "flex-1 overflow-hidden bg-base-100",
                    SpriteCanvas {
                        character: character.clone(),
                        character_name: character.name.clone(),
                        sprite_group_name: sprite_group.name.clone(),
                        draft,
                        history,
                        selected_sprite_index,
                        selected_box,
                        dragging,
                        references,
                        visibility,
                    }
                }
                div { class: "w-72 shrink-0",
                    SpritePropertyPanel {
                        character: character.clone(),
                        character_name: character.name.clone(),
                        sprite_group_name: sprite_group.name.clone(),
                        draft,
                        history,
                        selected_sprite_index,
                        selected_box,
                        disk_ops,
                        references,
                    }
                }
            }
        }
    }
}
