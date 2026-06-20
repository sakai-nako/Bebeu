use dioxus::prelude::*;

/// Project 一覧の再取得を誘発するためのカウンター。
///
/// - 提供側: `AppMain` で `use_projects_refresh_provider()` を 1 度呼ぶ
/// - 購読側: Project 一覧表示などの `use_effect` 内で `.subscribe()` し、
///   トリガーが上がるたびに repo.list() を再実行する
/// - 発火側: features 層 (Create/Delete/Rename 等) で `.bump()`
#[derive(Clone, Copy)]
pub struct ProjectsRefreshTrigger(Signal<u64>);

impl ProjectsRefreshTrigger {
    /// `use_effect` 内で呼ぶと、トリガー値の変化で effect が再実行されるようになる
    pub fn subscribe(&self) -> u64 {
        self.0.read().to_owned()
    }

    /// 値をインクリメントして購読者の再実行を誘発する
    pub fn bump(&mut self) {
        let next = self.0.read().wrapping_add(1);
        self.0.set(next);
    }
}

pub fn use_projects_refresh_provider() {
    use_context_provider(|| ProjectsRefreshTrigger(Signal::new(0)));
}

pub fn use_projects_refresh() -> ProjectsRefreshTrigger {
    use_context::<ProjectsRefreshTrigger>()
}
