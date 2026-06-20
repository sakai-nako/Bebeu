use dioxus::prelude::*;

use crate::shared::{use_image_cache_buster, versioned_asset_url, workspace_asset_url};

#[component]
pub fn SpriteThumbnail(
    character_name: String,
    sprite_group_name: String,
    index: u32,
    path: String,
) -> Element {
    let cache_buster = use_image_cache_buster();
    let version = cache_buster.map_or(0, |s| s.read().0);
    let url = versioned_asset_url(
        workspace_asset_url(&format!(
            "data/characters/{character_name}/sprite-groups/{sprite_group_name}/sprites/{path}"
        )),
        version,
    );

    rsx! {
        div { class: "space-y-1",
            img {
                src: "{url}",
                alt: "{path}",
                class: "w-full aspect-square object-contain rounded border border-base-300 bg-base-100",
            }
            p { class: "text-xs text-center text-base-content/60", "#{index} {path}" }
        }
    }
}
