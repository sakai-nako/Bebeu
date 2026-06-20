use dioxus::prelude::*;

use super::Action;

/// 「グローバル listener が解決した結果のアクション集合」を表す Signal の値。
///
/// `seq` は wrapping カウンタで、同じ集合を連続で発火しても効率的に検知できるようにする
/// (Refresh トリガー / ADR-0004 と同じ思想)。`actions` は 1 回の押下で resolve された
/// Action の集合。同一キーに複数 Action を割り当てた場合は両方が含まれ、listener 側で
/// `matches(target)` でフィルタする。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyboardActionRequest {
    pub seq: u64,
    pub actions: Vec<Action>,
}

impl KeyboardActionRequest {
    /// この request の actions に target が含まれているか。
    #[must_use]
    pub fn matches(&self, target: Action) -> bool {
        self.actions.contains(&target)
    }
}

/// グローバル listener から Action を発火する側、画面側 hook が読み取る側、両方に渡される
/// Signal ハンドル。Copy なので prop / closure キャプチャが容易。
#[derive(Clone, Copy)]
pub struct KeyboardActionDispatcher(Signal<KeyboardActionRequest>);

impl KeyboardActionDispatcher {
    /// Action 集合をまとめて 1 回の Signal 更新で発火する。`seq` が 1 増えるので、
    /// 同集合の連打でも use_effect が確実に起きる。
    /// 単一 Action を発火する場合は `vec![action]` を渡す。
    pub fn fire(&mut self, actions: Vec<Action>) {
        let next_seq = self.0.peek().seq.wrapping_add(1);
        self.0.set(KeyboardActionRequest {
            seq: next_seq,
            actions,
        });
    }

    /// 現在の値を読み取り、呼び出し scope を Signal に購読させる。
    /// `use_effect` の中で呼ぶことで Action 発火時に effect が再実行される。
    #[must_use]
    pub fn current(&self) -> KeyboardActionRequest {
        self.0.read().clone()
    }
}

/// `Signal<KeyboardActionRequest>` を Dioxus context で配布する。`AppMain` で 1 度だけ呼ぶ。
pub fn use_keyboard_action_provider() {
    use_context_provider(|| {
        KeyboardActionDispatcher(Signal::new(KeyboardActionRequest::default()))
    });
}

/// 配布済みの dispatcher を取得する。listener / 画面側 hook 両方から呼ぶ。
pub fn use_keyboard_action_dispatcher() -> KeyboardActionDispatcher {
    use_context::<KeyboardActionDispatcher>()
}
