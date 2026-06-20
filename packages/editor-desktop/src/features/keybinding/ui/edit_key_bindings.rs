use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::keybinding::{Action, KeyBindings};
use crate::entities::preference::{Preferences, PreferencesRepository, use_preferences};
use crate::shared::KeyBinding;

/// Preferences モーダル内の「キーボードショートカット」セクション。
///
/// 各 Action ごとに 1 行表示し、現在のキーバインドを `kbd` で見せる。「✎」ボタンを押すと
/// その行が capture モードになり、次のキー押下で binding を更新する (Esc で取消、競合時は赤字)。
/// 「↺」で個別アクションをデフォルトに戻し、「✕」で個別アクションをクリアする。
/// セクション上部の「すべてデフォルト」「すべてクリア」で全 Action を一括操作する。
#[component]
pub fn EditKeyBindings() -> Element {
    let repo = use_context::<Arc<dyn PreferencesRepository>>();
    let mut preferences = use_preferences();
    let mut bulk_error = use_signal(|| None::<String>);

    // 複数の onclick から呼ぶため Copy な `Callback` 化する。
    let apply_bindings = use_callback(move |next_bindings: KeyBindings| {
        let snapshot = preferences.peek().clone();
        let next_prefs = Preferences {
            key_bindings: next_bindings,
            ..snapshot
        };
        match repo.save(&next_prefs) {
            Ok(()) => {
                preferences.set(next_prefs);
                bulk_error.set(None);
            }
            Err(e) => bulk_error.set(Some(e.to_string())),
        }
    });

    let on_reset_all = move |_| apply_bindings.call(KeyBindings::default());
    let on_clear_all = move |_| {
        let mut next = preferences.peek().key_bindings.clone();
        next.clear_all();
        apply_bindings.call(next);
    };

    rsx! {
        fieldset { class: "fieldset",
            legend { class: "fieldset-legend", "キーボードショートカット" }
            div { class: "flex items-center gap-2 mb-2",
                button {
                    r#type: "button",
                    class: "btn btn-xs btn-outline",
                    onclick: on_reset_all,
                    "すべてデフォルトに戻す"
                }
                button {
                    r#type: "button",
                    class: "btn btn-xs btn-outline btn-error",
                    onclick: on_clear_all,
                    "すべてクリア"
                }
            }
            if let Some(message) = bulk_error() {
                p { class: "text-error text-xs mb-2", "{message}" }
            }
            div { class: "space-y-2",
                for action in Action::ALL.iter().copied() {
                    KeyBindingRow { key: "{action.id()}", action }
                }
            }
        }
    }
}

#[component]
fn KeyBindingRow(action: Action) -> Element {
    let repo = use_context::<Arc<dyn PreferencesRepository>>();
    let mut preferences = use_preferences();
    let mut capturing = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    // 動的に追加した要素では HTML の `autofocus` 属性が効かないので、自前 ID + document::eval で
    // フォーカスを当てる (app_root.rs の data-theme と同じ eval 手法)。
    let input_id = format!("kb-capture-{}", action.id());
    {
        let id_for_effect = input_id.clone();
        use_effect(move || {
            if capturing() {
                document::eval(&format!(
                    "document.getElementById('{id_for_effect}')?.focus()"
                ));
            }
        });
    }

    // `read()` で購読: 別の Action 編集時もここが再描画されるが、コストは無視できる
    let prefs_view = preferences.read();
    let current = prefs_view.key_bindings.get(action).cloned();
    let display = current
        .as_ref()
        .map_or_else(|| "(未設定)".to_string(), KeyBinding::to_string);
    drop(prefs_view);

    // 複数の onclick / capture から呼ぶため Copy な `Callback` 化する。
    // 「セット / 個別デフォルト / 個別クリア」共通の save → signal 更新ロジック。
    let apply_change = use_callback(move |next_bindings: KeyBindings| {
        let snapshot = preferences.peek().clone();
        let next_prefs = Preferences {
            key_bindings: next_bindings,
            ..snapshot
        };
        match repo.save(&next_prefs) {
            Ok(()) => {
                preferences.set(next_prefs);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    });

    let mut apply_binding = move |kb: KeyBinding| {
        let snapshot = preferences.peek().clone();
        let conflicts = snapshot.key_bindings.conflicts(&kb, action);
        if !conflicts.is_empty() {
            let names: Vec<&'static str> = conflicts.iter().map(|a| a.label()).collect();
            error.set(Some(format!(
                "「{kb}」 は {} と競合します",
                names.join(", ")
            )));
            return;
        }
        let mut next_bindings: KeyBindings = snapshot.key_bindings.clone();
        next_bindings.set(action, kb);
        apply_change.call(next_bindings);
    };

    let on_reset = move |_| {
        let snapshot = preferences.peek().clone();
        let kb = action.default_binding();
        // デフォルトキーが他 Action と競合する場合は警告を出して中止
        let conflicts = snapshot.key_bindings.conflicts(&kb, action);
        if !conflicts.is_empty() {
            let names: Vec<&'static str> = conflicts.iter().map(|a| a.label()).collect();
            error.set(Some(format!(
                "デフォルトの「{kb}」は {} と競合します",
                names.join(", ")
            )));
            return;
        }
        let mut next_bindings = snapshot.key_bindings.clone();
        next_bindings.set(action, kb);
        apply_change.call(next_bindings);
    };

    let on_clear = move |_| {
        let mut next_bindings = preferences.peek().key_bindings.clone();
        next_bindings.remove(action);
        apply_change.call(next_bindings);
    };

    rsx! {
        div { class: "flex items-center gap-2",
            span { class: "flex-1 text-sm", "{action.label()}" }
            if capturing() {
                input {
                    id: "{input_id}",
                    r#type: "text",
                    class: "input input-bordered input-sm w-40",
                    placeholder: "キーを押してください…",
                    readonly: true,
                    onkeydown: move |evt: KeyboardEvent| {
                        // Escape で取り消し
                        if matches!(evt.key(), dioxus::prelude::Key::Escape) {
                            evt.prevent_default();
                            capturing.set(false);
                            return;
                        }
                        if let Some(kb) = KeyBinding::from_keyboard_event(&evt) {
                            evt.prevent_default();
                            apply_binding(kb);
                            capturing.set(false);
                        }
                    },
                    onblur: move |_| capturing.set(false),
                }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: move |_| capturing.set(false),
                    "取消"
                }
            } else {
                kbd { class: "kbd kbd-sm", "{display}" }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    title: "再キャプチャ",
                    onclick: move |_| {
                        error.set(None);
                        capturing.set(true);
                    },
                    "✎"
                }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    title: "デフォルトに戻す",
                    onclick: on_reset,
                    "↺"
                }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    title: "クリア",
                    onclick: on_clear,
                    "✕"
                }
            }
        }
        if let Some(message) = error() {
            p { class: "text-error text-xs mt-1", "{message}" }
        }
    }
}
