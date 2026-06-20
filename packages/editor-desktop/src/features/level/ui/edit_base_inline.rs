use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::level::{Level, LevelRepository, use_levels_refresh};
use crate::shared::use_image_cache_buster;

/// `base` レイヤー画像のインライン編集。ファイルピッカーで画像を選び、即座に
/// `workspace/data/levels/{name}/base.{ext}` にコピーして `Level.base` を更新する。
#[component]
pub fn EditBaseInline(level: Level) -> Element {
    let repo = use_context::<Arc<dyn LevelRepository>>();
    let mut refresh = use_levels_refresh();
    let cache_buster = use_image_cache_buster();

    let mut error = use_signal(|| None::<String>);
    let original = level.clone();

    let on_pick_image = move |_| {
        let Some(source) = rfd::FileDialog::new()
            .set_title("Base 画像を選択")
            .add_filter("画像", &["png", "jpg", "jpeg", "webp", "bmp"])
            .pick_file()
        else {
            return;
        };
        let basename = match repo.import_base_image(&original.name, &source) {
            Ok(b) => b,
            Err(e) => {
                error.set(Some(e.to_string()));
                return;
            }
        };
        let updated = Level {
            base: basename,
            ..original.clone()
        };
        match repo.save(&updated) {
            Ok(()) => {
                refresh.bump();
                if let Some(mut cb) = cache_buster {
                    cb.write().bump();
                }
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    rsx! {
        div { class: "flex items-center gap-2",
            span { class: "font-mono text-sm", "{level.base}" }
            button {
                r#type: "button",
                class: "btn btn-ghost btn-xs",
                onclick: on_pick_image,
                title: "画像を選択して差し替え",
                "✎ 変更"
            }
        }
        if let Some(message) = error() {
            p { class: "text-error text-xs mt-1", "{message}" }
        }
    }
}
