//! 全画面共通のトースト通知システム。
//!
//! ## 用途
//!
//! アクション結果のフィードバック (取り込み枚数の報告 / Repository エラー / モーダル
//! が閉じてから残したい一過性メッセージ) を画面端の daisyUI `toast` で表示する。
//! フォーム内バリデーションエラーや静的な注意書きは引き続きインラインの `alert` を使うこと
//! (→ `features/character/ui/README.md`)。
//!
//! ## 使い方
//!
//! 1. app ツリーのトップで `use_toast_provider()` を 1 度呼び、ツリーのどこかに
//!    `ToastHost {}` を mount しておく
//! 2. 通知を出したいコンポーネントで `use_toast()` を呼ぶ:
//!    ```ignore
//!    let mut toast = use_toast();
//!    toast.success(format!("{n} 枚を取り込みました"));
//!    toast.error(e.to_string());
//!    ```
//!
//! ## 自動消滅 (CSS アニメーション + animationend)
//!
//! Dioxus の async runtime を避ける方針 (ADR-0002) のため setTimeout は使わず、
//! `theme.css` の `toast-auto-dismiss` keyframe で 4 秒かけてフェードアウトさせ、
//! `onanimationend` で `queue.dismiss(id)` を呼ぶ。Success / Info は自動消滅、
//! Error / Warning は手動で `×` を押すまで残す (見落とし防止)。
//! トーストにホバーすると `animation-play-state: paused` で消滅が一時停止する。
//! 設計の経緯は ADR-0013 を参照。

use std::collections::VecDeque;

use dioxus::prelude::*;

/// トーストの種別。daisyUI の alert カラーに対応。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastKind {
    fn alert_class(self) -> &'static str {
        match self {
            ToastKind::Success => "alert alert-success",
            ToastKind::Error => "alert alert-error",
            ToastKind::Warning => "alert alert-warning",
            ToastKind::Info => "alert alert-info",
        }
    }

    /// 自動消滅対象なら true。Error / Warning は重要度が高いので残す。
    fn auto_dismiss(self) -> bool {
        matches!(self, ToastKind::Success | ToastKind::Info)
    }
}

/// 1 件のトースト。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
}

/// 表示中トーストの待ち行列。
#[derive(Debug, Clone, Default)]
pub struct ToastQueue {
    next_id: u64,
    items: VecDeque<Toast>,
}

impl ToastQueue {
    /// 末尾に追加し、割り当てた id を返す。
    pub fn push(&mut self, kind: ToastKind, message: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.items.push_back(Toast {
            id,
            kind,
            message: message.into(),
        });
        id
    }

    /// 指定 id を 1 件削除する。存在しなければ何もしない。
    pub fn dismiss(&mut self, id: u64) {
        self.items.retain(|t| t.id != id);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &Toast> {
        self.items.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// トースト発火用の Copy ハンドル。子コンポーネントは `use_toast()` で取得し、
/// `success` / `error` / `warning` / `info` を呼ぶだけで通知を出せる。
#[derive(Clone, Copy)]
pub struct UseToast {
    queue: Signal<ToastQueue>,
}

impl PartialEq for UseToast {
    fn eq(&self, other: &Self) -> bool {
        self.queue == other.queue
    }
}

impl UseToast {
    pub fn success(&mut self, message: impl Into<String>) {
        self.queue.write().push(ToastKind::Success, message);
    }
    pub fn error(&mut self, message: impl Into<String>) {
        self.queue.write().push(ToastKind::Error, message);
    }
    pub fn warning(&mut self, message: impl Into<String>) {
        self.queue.write().push(ToastKind::Warning, message);
    }
    pub fn info(&mut self, message: impl Into<String>) {
        self.queue.write().push(ToastKind::Info, message);
    }
}

/// app ツリーのトップで 1 度だけ呼び、`ToastHost` と `use_toast` の両方が見る
/// 共有 queue を context に置く。
pub fn use_toast_provider() -> Signal<ToastQueue> {
    use_context_provider(|| Signal::new(ToastQueue::default()))
}

/// 子コンポーネントから通知を出すための Hook。`use_toast_provider` が居る scope で使う。
#[must_use]
pub fn use_toast() -> UseToast {
    let queue = use_context::<Signal<ToastQueue>>();
    UseToast { queue }
}

/// 画面右下にトースト一覧を描画する。app ツリーに 1 つだけ mount する想定。
#[component]
pub fn ToastHost() -> Element {
    let queue = use_context::<Signal<ToastQueue>>();
    rsx! {
        div { class: "toast toast-end z-50",
            for toast in queue.read().iter() {
                ToastItem {
                    key: "{toast.id}",
                    id: toast.id,
                    kind: toast.kind,
                    message: toast.message.clone(),
                }
            }
        }
    }
}

#[component]
fn ToastItem(id: u64, kind: ToastKind, message: String) -> Element {
    let mut queue = use_context::<Signal<ToastQueue>>();
    let alert_class = kind.alert_class();
    let auto_dismiss = kind.auto_dismiss();
    // opacity-90 は常時透過 (theme テーマと馴染ませる)。toast-auto-dismiss クラスが付く
    // ものは 4s 後に CSS アニメーションで消え、onanimationend で state からも除去する。
    let class = if auto_dismiss {
        format!("{alert_class} opacity-90 shadow-lg toast-auto-dismiss")
    } else {
        format!("{alert_class} opacity-90 shadow-lg")
    };
    rsx! {
        div {
            role: "alert",
            class: "{class}",
            onanimationend: move |_| {
                // 自動消滅クラスのアニメ完了時のみ dismiss。daisyUI 内蔵のトランジション
                // (animation ではない) は animationend を発火しないので、追加の name チェックは省略。
                if auto_dismiss {
                    queue.write().dismiss(id);
                }
            },
            span { class: "flex-1", "{message}" }
            button {
                r#type: "button",
                class: "btn btn-ghost btn-xs min-h-0 h-5 px-1",
                title: "閉じる",
                onclick: move |_| queue.write().dismiss(id),
                "×"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_assigns_unique_ids_and_preserves_order() {
        let mut q = ToastQueue::default();
        let id1 = q.push(ToastKind::Success, "a");
        let id2 = q.push(ToastKind::Error, "b");
        assert_ne!(id1, id2);
        assert_eq!(q.len(), 2);
        let items: Vec<_> = q.iter().collect();
        assert_eq!(items[0].message, "a");
        assert_eq!(items[1].message, "b");
    }

    #[test]
    fn dismiss_removes_only_matching_id() {
        let mut q = ToastQueue::default();
        let id1 = q.push(ToastKind::Info, "a");
        let id2 = q.push(ToastKind::Info, "b");
        q.dismiss(id1);
        assert_eq!(q.len(), 1);
        assert_eq!(q.iter().next().expect("queue should have one item").id, id2);
    }

    #[test]
    fn dismiss_unknown_id_is_noop() {
        let mut q = ToastQueue::default();
        let _ = q.push(ToastKind::Info, "a");
        q.dismiss(999);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn clear_empties_queue() {
        let mut q = ToastQueue::default();
        let _ = q.push(ToastKind::Info, "a");
        let _ = q.push(ToastKind::Info, "b");
        q.clear();
        assert!(q.is_empty());
    }

    #[test]
    fn ids_wrap_on_overflow() {
        let mut q = ToastQueue {
            next_id: u64::MAX,
            items: VecDeque::new(),
        };
        let id1 = q.push(ToastKind::Info, "a");
        let id2 = q.push(ToastKind::Info, "b");
        assert_eq!(id1, u64::MAX);
        assert_eq!(id2, 0);
    }

    #[test]
    fn alert_class_matches_kind() {
        assert_eq!(ToastKind::Success.alert_class(), "alert alert-success");
        assert_eq!(ToastKind::Error.alert_class(), "alert alert-error");
        assert_eq!(ToastKind::Warning.alert_class(), "alert alert-warning");
        assert_eq!(ToastKind::Info.alert_class(), "alert alert-info");
    }

    #[test]
    fn auto_dismiss_only_for_success_and_info() {
        assert!(ToastKind::Success.auto_dismiss());
        assert!(ToastKind::Info.auto_dismiss());
        assert!(!ToastKind::Error.auto_dismiss());
        assert!(!ToastKind::Warning.auto_dismiss());
    }
}
