use dioxus::prelude::*;

use crate::widgets::project::ProjectDetail;

#[component]
pub fn ProjectsIndex() -> Element {
    rsx! {
        div { class: "h-full flex items-center justify-center text-base-content/50",
            "サイドバーから Project を選択してください。"
        }
    }
}

/// `/projects/:name` の controller。Project 名を URL から受け取って詳細編集 UI を表示する。
#[component]
pub fn ProjectDetailPage(name: ReadSignal<String>) -> Element {
    rsx! {
        ProjectDetail { target_name: name }
    }
}
