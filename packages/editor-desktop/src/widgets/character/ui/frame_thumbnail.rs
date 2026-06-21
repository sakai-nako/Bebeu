use dioxus::prelude::*;

use crate::entities::character::{Character, Frame};
use crate::shared::{use_image_cache_buster, versioned_asset_url, workspace_asset_url};

/// 1 frame の小さなプレビューカード。layers を横並列に並べ、各 layer は
/// `Character::find_sprite` で sprite を解決して thumbnail を描画する（合成はしない）。
#[component]
pub fn FrameThumbnail(character: Character, frame: Frame) -> Element {
    rsx! {
        div { class: "card bg-base-100 border border-base-300 rounded-box p-2 space-y-1",
            div { class: "flex items-center justify-between gap-2 text-xs",
                span { class: "font-mono font-semibold", "#{frame.index}" }
                span { class: "text-base-content/60", "{frame.ticks}t" }
            }
            div { class: "flex flex-row flex-wrap gap-1",
                for layer in frame.layers.iter() {
                    LayerThumbnail {
                        key: "{layer.index}",
                        character: character.clone(),
                        layer: layer.clone(),
                    }
                }
            }
        }
    }
}

#[component]
fn LayerThumbnail(character: Character, layer: crate::entities::character::Layer) -> Element {
    let resolved = character.find_sprite(layer.sprite_group_number, layer.sprite_index);
    // 画像差し替え後の WebView キャッシュ起因の古画像表示を避けるため versioned URL を使う。
    let cache_buster = use_image_cache_buster();
    let version = cache_buster.map_or(0, |s| s.read().0);

    rsx! {
        if let Some((group, sprite)) = resolved {
            {
                let url = versioned_asset_url(
                    workspace_asset_url(
                        &format!(
                            "data/characters/{}/sprite-groups/{}/sprites/{}",
                            character.name,
                            group.name,
                            sprite.path,
                        ),
                    ),
                    version,
                );
                rsx! {
                    img {
                        src: "{url}",
                        alt: "layer #{layer.index}",
                        class: "w-12 h-12 object-contain rounded border border-base-300 bg-base-200",
                        style: "opacity: {layer.transparency}",
                    }
                }
            }
        } else {
            div {
                class: "w-12 h-12 flex items-center justify-center rounded border border-error/40 bg-error/10 text-[10px] text-error",
                title: "SpriteGroup #{layer.sprite_group_number}, sprite_index {layer.sprite_index} が見つかりません",
                "missing"
            }
        }
    }
}
