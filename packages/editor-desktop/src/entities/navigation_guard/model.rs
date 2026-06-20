use dioxus::prelude::*;
use dioxus::router::Navigator;

/// 「未保存の変更がある画面」から離脱しようとしたときに確認ダイアログを表示するための
/// グローバル state。
///
/// - `blocked`: 編集中コンポーネントが「未保存」を宣言する Signal
/// - `pending`: ナビゲーション要求された URL の保留先 (confirm 待ち)
///
/// ナビゲーション要素 (左 rail / breadcrumb / Cancel ボタン等) は `try_navigate` 経由で
/// 移動先を要求し、`blocked == true` のときは `pending` に積んで RootShell の confirm
/// ダイアログで「破棄して移動」/「やめる」を待つ。
#[derive(Clone, Copy)]
pub struct NavigationGuard {
    blocked: Signal<bool>,
    pending: Signal<Option<String>>,
}

impl NavigationGuard {
    /// 編集中コンポーネントが未保存状態を宣言する。
    /// 値が変わらないときは Signal を更新しない (不要な再レンダを避ける)。
    pub fn set_blocked(&mut self, blocked: bool) {
        if *self.blocked.peek() != blocked {
            self.blocked.set(blocked);
        }
    }

    /// 現在 navigation がブロックされているか
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        *self.blocked.read()
    }

    /// confirm ダイアログ表示用に pending URL を取得する (Signal を購読する)。
    #[must_use]
    pub fn pending(&self) -> Option<String> {
        self.pending.read().clone()
    }

    /// 移動先 URL を要求する。`blocked` のときは `pending` に積んで confirm を待つ、
    /// そうでないときは即座に `nav.push` する。
    pub fn try_navigate(&mut self, nav: &Navigator, route: String) {
        if self.is_blocked() {
            self.pending.set(Some(route));
        } else {
            nav.push(route);
        }
    }

    /// confirm ダイアログで「破棄して移動」を選んだとき呼ぶ。
    /// `pending` の URL に navigate して、blocked 状態は解除する (= 編集破棄したので)。
    pub fn confirm(&mut self, nav: &Navigator) {
        let route = self.pending.peek().clone();
        self.pending.set(None);
        self.blocked.set(false);
        if let Some(route) = route {
            nav.push(route);
        }
    }

    /// confirm ダイアログで「やめる」を選んだとき呼ぶ。`pending` を消すだけ。
    pub fn cancel(&mut self) {
        self.pending.set(None);
    }
}

/// Dioxus context に `NavigationGuard` を 1 度だけ配布する。`AppMain` で呼ぶ。
pub fn use_navigation_guard_provider() {
    use_context_provider(|| NavigationGuard {
        blocked: Signal::new(false),
        pending: Signal::new(None),
    });
}

/// 配布済みの `NavigationGuard` を取得する。Link / Cancel ボタン / 編集中コンポーネントから呼ぶ。
pub fn use_navigation_guard() -> NavigationGuard {
    use_context::<NavigationGuard>()
}
