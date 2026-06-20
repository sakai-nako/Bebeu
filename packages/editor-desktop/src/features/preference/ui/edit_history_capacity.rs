use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::preference::{Preferences, PreferencesRepository, use_preferences};

/// 上限値の妥当範囲。1 未満は履歴無効、500 超えは Sprite 数件 × フィールド数のスナップショットを
/// 大量に持つことになりメモリ的に過剰。実用上 50〜200 で十分。
const MIN_CAPACITY: u32 = 1;
const MAX_CAPACITY: u32 = 500;

#[component]
pub fn EditHistoryCapacity() -> Element {
    let repo = use_context::<Arc<dyn PreferencesRepository>>();
    let mut preferences = use_preferences();
    let mut error = use_signal(|| None::<String>);

    let current = preferences.read().sprite_group_history_capacity;
    // 入力値は文字列で持つ。u32 直接バインドは編集途中で 0 に丸まる問題があるため
    // (features/character/ui/README.md「数値入力は String で持つ」参照)。
    let mut input = use_signal(|| current.to_string());

    // 外部 (ResetPreferencesButton 等) で current が変わった時に input も追従させる。
    // 自分の onchange からの set でも発火するが、同値 set は実害なし。
    use_effect(move || {
        let latest = preferences.read().sprite_group_history_capacity;
        input.set(latest.to_string());
    });

    let on_change = move |evt: Event<FormData>| {
        let raw = evt.value();
        let Ok(parsed) = raw.trim().parse::<u32>() else {
            error.set(Some(format!(
                "{MIN_CAPACITY} 〜 {MAX_CAPACITY} の整数で入力してください",
            )));
            return;
        };
        if !(MIN_CAPACITY..=MAX_CAPACITY).contains(&parsed) {
            error.set(Some(format!(
                "{MIN_CAPACITY} 〜 {MAX_CAPACITY} の整数で入力してください",
            )));
            return;
        }
        let new_prefs = Preferences {
            sprite_group_history_capacity: parsed,
            ..preferences.peek().clone()
        };
        // disk 保存に成功してから signal を更新 (disk と memory の乖離を避ける)
        match repo.save(&new_prefs) {
            Ok(()) => {
                preferences.set(new_prefs);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    rsx! {
        fieldset { class: "fieldset",
            legend { class: "fieldset-legend", "Undo 履歴の上限" }
            input {
                r#type: "number",
                class: "input input-bordered w-full",
                min: i64::from(MIN_CAPACITY),
                max: i64::from(MAX_CAPACITY),
                value: "{input}",
                oninput: move |evt| input.set(evt.value()),
                onchange: on_change,
            }
            p { class: "text-xs text-base-content/60 mt-1",
                "SpriteGroup Editor で記録する Undo の最大ステップ数。変更は次回編集画面を開いた時から反映されます。"
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        }
    }
}
