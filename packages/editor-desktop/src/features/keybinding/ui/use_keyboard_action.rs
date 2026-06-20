use dioxus::prelude::*;

use crate::entities::keybinding::{Action, use_keyboard_action_dispatcher};

/// グローバルにディスパッチされた Action を購読し、`target` と一致するときに `handler` を呼ぶ。
///
/// コンポーネントが unmount すると use_effect 購読ごと自然に消えるので、listener 解除の
/// クリーンアップは不要。「画面がアクティブな間だけ Action を処理する」が自然に成立する。
///
/// 内部で `last_seen_seq` を保持して、初期 seq=0 の誤発火を防止しつつ、同じ Action の
/// 連打 (seq だけ進む) でも確実に handler を呼ぶ。
pub fn use_keyboard_action<F>(target: Action, mut handler: F)
where
    F: FnMut() + 'static,
{
    let dispatcher = use_keyboard_action_dispatcher();
    let mut last_seen = use_signal(|| 0_u64);
    use_effect(move || {
        let req = dispatcher.current();
        // 初期状態 (seq=0, actions=[]) は通さない。target が含まれ、seq が更新されたら呼ぶ。
        if req.matches(target) && req.seq != *last_seen.peek() {
            last_seen.set(req.seq);
            handler();
        }
    });
}
