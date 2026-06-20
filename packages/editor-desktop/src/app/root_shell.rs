use dioxus::prelude::*;

use super::routes::Routes;
use crate::entities::keybinding::use_keyboard_action_dispatcher;
use crate::entities::navigation_guard::use_navigation_guard;
use crate::entities::preference::use_preferences;
use crate::shared::{KeyBinding, use_image_cache_buster_provider};
use crate::widgets::preference::PreferencesModal;

#[component]
pub fn RootShell() -> Element {
    let mut show_preferences = use_signal(|| false);
    let preferences = use_preferences();
    let mut dispatcher = use_keyboard_action_dispatcher();
    let mut guard = use_navigation_guard();
    // 画像 URL のキャッシュバスタを app 全体で共有する。SpriteGroupEditor 等で bump すると、
    // 別 Editor (AnimationEditor 等) で同じ画像 URL を表示する際にも `?v={N}` が反映されて
    // WebView の HTTP キャッシュを回避できる。Editor を跨いだ古画像表示の対策。
    let _image_cache_buster = use_image_cache_buster_provider();
    let nav = use_navigator();
    let current = use_route::<Routes>();
    let characters_active = matches!(
        current,
        Routes::CharactersIndex {}
            | Routes::CharacterDetailPage { .. }
            | Routes::SpriteGroupEditorPage { .. }
            | Routes::AnimationEditorPage { .. }
    );
    let levels_active = matches!(
        current,
        Routes::LevelsIndex {} | Routes::LevelDetailPage { .. }
    );
    let projects_active = matches!(
        current,
        Routes::ProjectsIndex {} | Routes::ProjectDetailPage { .. }
    );

    // モーダルを閉じたあとなどに focus が <body> に落ちると、root div の onkeydown まで
    // bubble せずショートカットが効かなくなる。focusout を監視して、入力可能要素以外で
    // focus が消えたら root へ戻す guard を JS 側に常駐させる。
    use_effect(|| {
        document::eval(
            r#"
            if (!window.__appFocusGuardInstalled) {
                window.__appFocusGuardInstalled = true;
                document.addEventListener("focusout", () => {
                    // focus 遷移は同期的に発火するため、次のタスクで activeElement を確認する
                    setTimeout(() => {
                        const a = document.activeElement;
                        if (!a || a === document.body) {
                            const root = document.getElementById("app-root");
                            if (root) root.focus();
                        }
                    }, 0);
                });
            }
            "#,
        );
    });

    // グローバルキーボードショートカット: 押下キーを Action に解決して dispatcher に流す。
    // tabindex=-1 + autofocus でフォーカスを掴むことで、起動直後から onkeydown が動く。
    let on_global_keydown = move |evt: KeyboardEvent| {
        let Some(kb) = KeyBinding::from_keyboard_event(&evt) else {
            return;
        };
        let actions = preferences.read().key_bindings.resolve(&kb);
        if actions.is_empty() {
            return;
        }
        // ブラウザ既定動作 (例: Ctrl+S の保存ダイアログ) を抑止
        evt.prevent_default();
        // 同一キーに複数 Action が割り当てられている場合も、Vec をまとめて 1 回の Signal 更新
        // で発火する。連続 fire で Signal がバッチングされ最後の Action しか届かない問題を防ぐ。
        // listener 側は target Action にしか反応しないので、active な editor だけが処理する。
        dispatcher.fire(actions);
    };

    rsx! {
        div {
            id: "app-root",
            class: "flex h-screen",
            tabindex: "-1",
            autofocus: true,
            onkeydown: on_global_keydown,
            // 左 rail
            aside { class: "w-20 bg-base-300 flex flex-col items-center py-4 gap-3 border-r border-base-300 shrink-0",
                // 上部: Characters (guard 経由のナビ)
                button {
                    r#type: "button",
                    class: rail_button_class(characters_active),
                    title: "Characters",
                    onclick: move |_| guard.try_navigate(&nav, "/characters".to_string()),
                    UsersIcon {}
                }

                // Levels (master pool の Stage / 舞台)
                button {
                    r#type: "button",
                    class: rail_button_class(levels_active),
                    title: "Levels",
                    onclick: move |_| guard.try_navigate(&nav, "/levels".to_string()),
                    MapIcon {}
                }

                // Projects (workspace 内の複数プロジェクト管理)
                button {
                    r#type: "button",
                    class: rail_button_class(projects_active),
                    title: "Projects",
                    onclick: move |_| guard.try_navigate(&nav, "/projects".to_string()),
                    FolderIcon {}
                }

                // スペーサー
                div { class: "flex-1" }

                // 下部: Preferences (ユーザー設定)
                button {
                    r#type: "button",
                    class: rail_button_class(false),
                    title: "Preferences",
                    onclick: move |_| show_preferences.set(true),
                    GearIcon {}
                }
            }

            // メインエリア（ネストされた layout の Outlet）
            div { class: "flex-1 min-w-0 overflow-auto", Outlet::<Routes> {} }
        }

        if show_preferences() {
            PreferencesModal { onclose: move |()| show_preferences.set(false) }
        }

        // 未保存変更の破棄確認 (左 rail / breadcrumb / Cancel ボタン共通)
        if guard.pending().is_some() {
            dialog { class: "modal modal-open",
                div { class: "modal-box",
                    h3 { class: "text-lg font-bold mb-2", "編集を破棄しますか？" }
                    p { class: "py-2",
                        "未保存の変更があります。破棄して移動しますか？"
                    }
                    div { class: "modal-action",
                        button {
                            r#type: "button",
                            class: "btn btn-ghost",
                            onclick: move |_| guard.cancel(),
                            "やめる"
                        }
                        button {
                            r#type: "button",
                            class: "btn btn-error",
                            onclick: move |_| guard.confirm(&nav),
                            "破棄して移動"
                        }
                    }
                }
                div { class: "modal-backdrop",
                    button { r#type: "button", onclick: move |_| guard.cancel(), "close" }
                }
            }
        }
    }
}

fn rail_button_class(active: bool) -> &'static str {
    if active {
        "btn btn-square btn-md bg-primary text-primary-content hover:bg-primary"
    } else {
        "btn btn-square btn-md btn-ghost"
    }
}

// ── Inline SVG icons (heroicons / lucide 風) ──

#[component]
fn UsersIcon() -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            width: "24",
            height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" }
            circle { cx: "8.5", cy: "7", r: "4" }
            path { d: "M22 21v-2a4 4 0 0 0-3-3.87" }
            path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
        }
    }
}

#[component]
fn MapIcon() -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            width: "24",
            height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polygon { points: "3 6 9 3 15 6 21 3 21 18 15 21 9 18 3 21" }
            line {
                x1: "9",
                y1: "3",
                x2: "9",
                y2: "18",
            }
            line {
                x1: "15",
                y1: "6",
                x2: "15",
                y2: "21",
            }
        }
    }
}

#[component]
fn FolderIcon() -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            width: "24",
            height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M20 20H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h5l2 2h9a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2Z" }
        }
    }
}

#[component]
fn GearIcon() -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            width: "24",
            height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z" }
            path { d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1Z" }
        }
    }
}
