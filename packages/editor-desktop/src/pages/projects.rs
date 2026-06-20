use dioxus::prelude::*;

use crate::shared::Config;
use crate::widgets::project::{ProjectDetail, ProjectList};

/// `/projects` の controller。一覧 widget を表示するだけの thin component。
#[component]
pub fn ProjectsIndex() -> Element {
    rsx! {
        ProjectList {}
    }
}

/// `/projects/:name` の controller。Project 名を URL から受け取って詳細編集 UI を表示する。
#[component]
pub fn ProjectDetailPage(name: ReadSignal<String>) -> Element {
    // engine 起動コマンド表示用に workspace_dir を Config から再ロードする。
    // (Config::load() は I/O だが debug 時の CWD/release 時の exe-dir 解決のみで安価)
    let workspace_dir = use_signal(|| {
        Config::load()
            .map(|c| c.workspace_dir().display().to_string())
            .unwrap_or_default()
    });
    let workspace_signal: ReadSignal<String> = ReadSignal::new(workspace_dir);

    rsx! {
        ProjectDetail { target_name: name, workspace_dir: workspace_signal }
    }
}
