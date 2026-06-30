use dioxus::prelude::*;

use crate::entities::preference::use_t;
use crate::widgets::project::ProjectDetail;

#[component]
pub fn ProjectsIndex() -> Element {
    let t = use_t();
    rsx! {
        div { class: "h-full flex items-center justify-center text-base-content/50",
            "{t(\"projects.index_empty_hint\")}"
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
