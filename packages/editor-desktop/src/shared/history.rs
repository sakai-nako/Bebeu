use dioxus::prelude::*;

/// Undo / Redo 用の履歴スタック。
///
/// `past` は古い順、末尾が「直前のスナップショット」。`future` は redo 用で、末尾が「次に redo
/// したときに復元するスナップショット」。`record` 時に `future` をクリアするので分岐は発生しない。
///
/// `capacity` を超えた古い past は先頭から捨てる (リングバッファ的な挙動)。
#[derive(Debug, Clone)]
pub struct History<T> {
    past: Vec<T>,
    future: Vec<T>,
    capacity: usize,
}

impl<T: Clone> History<T> {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// 「これから変更を加える直前の状態」を 1 件記録する。
    /// 新しい変更が入ったので future は捨てる。
    pub fn record(&mut self, snapshot: T) {
        self.past.push(snapshot);
        self.future.clear();
        if self.past.len() > self.capacity {
            // 古い順に捨てるので先頭から remove。capacity は実用上 50 程度なのでコストは無視できる。
            self.past.remove(0);
        }
    }

    /// 1 つ前の状態を取り出して返す。
    /// 呼び出し側が現在値 `current` を渡すので、それは future 側にプッシュして redo 可能にする。
    pub fn undo(&mut self, current: T) -> Option<T> {
        let prev = self.past.pop()?;
        self.future.push(current);
        Some(prev)
    }

    /// 1 つ先の状態を取り出して返す。`current` は past 側にプッシュする。
    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.future.pop()?;
        self.past.push(current);
        Some(next)
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

/// Signal<History<T>> と編集対象の Signal<T> をペアで保持する Copy ハンドル。
///
/// `record` / `undo` / `redo` は `target` から peek した値を使うので、子コンポーネントは
/// この 1 ハンドルだけ受け取れば「変更前に履歴を取る」「Undo で戻す」操作が完結する。
pub struct UseHistory<T: 'static> {
    history: Signal<History<T>>,
    target: Signal<T>,
}

impl<T> Clone for UseHistory<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for UseHistory<T> {}

impl<T> PartialEq for UseHistory<T> {
    fn eq(&self, other: &Self) -> bool {
        // Signal は内部 GenerationalRef のアドレス比較で eq するので T: PartialEq は不要。
        self.history == other.history && self.target == other.target
    }
}

impl<T: Clone + 'static> UseHistory<T> {
    /// 現在の `target` 値をスナップショットとして履歴に積む。変更を加える直前に呼ぶ。
    pub fn record(&mut self) {
        let snapshot = self.target.peek().clone();
        self.history.write().record(snapshot);
    }

    /// 直前の状態に戻す。past が空なら何もしない。
    pub fn undo(&mut self) {
        let current = self.target.peek().clone();
        if let Some(prev) = self.history.write().undo(current) {
            self.target.set(prev);
        }
    }

    /// undo を取り消す。future が空なら何もしない。
    pub fn redo(&mut self) {
        let current = self.target.peek().clone();
        if let Some(next) = self.history.write().redo(current) {
            self.target.set(next);
        }
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.read().can_undo()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.read().can_redo()
    }
}

/// `target` Signal にひも付いた履歴スタックを Hook として作る。
/// `capacity` は保持する past の最大件数。
#[must_use]
pub fn use_history<T: Clone + 'static>(target: Signal<T>, capacity: usize) -> UseHistory<T> {
    let history = use_signal(|| History::<T>::new(capacity));
    UseHistory { history, target }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_then_undo_returns_previous_snapshot() {
        let mut h = History::<i32>::new(10);
        h.record(1);
        let prev = h.undo(2);
        assert_eq!(prev, Some(1));
        assert!(!h.can_undo());
        assert!(h.can_redo());
    }

    #[test]
    fn redo_restores_after_undo() {
        let mut h = History::<i32>::new(10);
        h.record(1);
        let _ = h.undo(2);
        let next = h.redo(1);
        assert_eq!(next, Some(2));
        assert!(h.can_undo());
        assert!(!h.can_redo());
    }

    #[test]
    fn record_after_undo_clears_future() {
        let mut h = History::<i32>::new(10);
        h.record(1);
        let _ = h.undo(2); // current=2 を future に積む
        assert!(h.can_redo());
        h.record(3); // 新規分岐 → future は捨てる
        assert!(!h.can_redo());
    }

    #[test]
    fn capacity_drops_oldest() {
        let mut h = History::<i32>::new(2);
        h.record(1);
        h.record(2);
        h.record(3); // 1 が押し出される
        assert_eq!(h.undo(4), Some(3));
        assert_eq!(h.undo(3), Some(2));
        assert_eq!(h.undo(2), None);
    }

    #[test]
    fn undo_on_empty_returns_none() {
        let mut h = History::<i32>::new(5);
        assert_eq!(h.undo(42), None);
    }

    #[test]
    fn redo_on_empty_returns_none() {
        let mut h = History::<i32>::new(5);
        assert_eq!(h.redo(42), None);
    }
}
