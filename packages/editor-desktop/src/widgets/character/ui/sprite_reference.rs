use dioxus::prelude::*;

use crate::entities::character::Character;
use crate::shared::{use_image_cache_buster, versioned_asset_url, workspace_asset_url};

/// 編集中の Sprite に対する Reference の重ね順。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferencePlacement {
    /// 編集中 Sprite の手前に重ねる
    Front,
    /// 編集中 Sprite の奥に重ねる
    Back,
}

impl ReferencePlacement {
    fn as_str(self) -> &'static str {
        match self {
            ReferencePlacement::Front => "front",
            ReferencePlacement::Back => "back",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "front" => ReferencePlacement::Front,
            _ => ReferencePlacement::Back,
        }
    }
}

/// 編集中 Sprite と pivot を揃えて重ね描く参照画像の設定。
/// editor のセッション内のみ保持し、disk には書き込まない。
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteReference {
    pub sprite_group_number: u32,
    pub sprite_index: u32,
    pub placement: ReferencePlacement,
    /// 0.0 〜 1.0。1.0 で完全不透明。
    pub opacity: f32,
}

impl SpriteReference {
    pub const DEFAULT_OPACITY: f32 = 0.5;
}

/// 親 container の (0, 0) を「編集中 Sprite の pivot 位置」とみなし、
/// reference 画像の pivot がその原点に重なるように `<img>` を絶対配置する。
///
/// 親側で「pivot 位置を (0, 0) に来るように配置した container」を用意して、その中で呼ぶ。
/// SpriteCanvas / AnimationCanvas で container の作り方が異なるため共通化していない。
///
/// `zoom` で reference 画像の表示倍率を指定する (4K + 150% 対策で wrapper の transform: scale を
/// 廃止したため、reference 側でも CSS px に zoom を直接乗算して explicit に寸法を出す)。
#[component]
pub fn ReferenceLayer(character: Character, reference: SpriteReference, zoom: f64) -> Element {
    let resolved = character.find_sprite(reference.sprite_group_number, reference.sprite_index);
    let Some((group, sprite)) = resolved else {
        return rsx! {
            span {
                class: "absolute text-[10px] text-error font-mono px-1 pointer-events-none whitespace-nowrap",
                style: "left: 0; top: 0;",
                "ref missing #{reference.sprite_group_number}/{reference.sprite_index}"
            }
        };
    };

    let cache_buster = use_image_cache_buster();
    let version = cache_buster.map_or(0, |s| s.read().0);
    let url = versioned_asset_url(
        workspace_asset_url(&format!(
            "data/characters/{}/sprite-groups/{}/sprites/{}",
            character.name, group.name, sprite.path,
        )),
        version,
    );
    let dx = f64::from(-sprite.pivot_point[0]) * zoom;
    let dy = f64::from(-sprite.pivot_point[1]) * zoom;
    let (w_zoomed, h_zoomed) = sprite.dimensions.map_or((0.0_f64, 0.0_f64), |[w, h]| {
        (f64::from(w) * zoom, f64::from(h) * zoom)
    });
    let opacity = reference.opacity.clamp(0.0, 1.0);

    rsx! {
        // `max-width: none` で Tailwind preflight の `max-width: 100%` を打ち消す。
        // 親 container は absolute で 0×0 のため、preflight をそのまま許すと img が消える。
        img {
            src: "{url}",
            class: "absolute pointer-events-none block",
            draggable: false,
            style: "left: {dx}px; top: {dy}px; width: {w_zoomed}px; height: {h_zoomed}px; max-width: none; opacity: {opacity}; image-rendering: pixelated;",
        }
    }
}

/// プロパティパネル末尾に置く Reference 設定セクション。
#[component]
pub fn ReferenceSection(
    character: Character,
    mut references: Signal<Vec<SpriteReference>>,
) -> Element {
    // 新規 Reference のデフォルト参照は Character の最初の SpriteGroup / Sprite から取る。
    let (default_group, default_sprite) = character.sprite_groups.first().map_or((0, 0), |g| {
        (g.number, g.sprites.first().map_or(0, |s| s.index))
    });
    let has_any_sprite = character
        .sprite_groups
        .iter()
        .any(|g| !g.sprites.is_empty());

    let on_add = move |_| {
        let mut updated = references();
        updated.push(SpriteReference {
            sprite_group_number: default_group,
            sprite_index: default_sprite,
            placement: ReferencePlacement::Back,
            opacity: SpriteReference::DEFAULT_OPACITY,
        });
        references.set(updated);
    };

    let snapshot = references();
    let count = snapshot.len();

    rsx! {
        div { class: "space-y-2",
            div { class: "flex items-center gap-2",
                h3 { class: "font-semibold", "References ({count})" }
                button {
                    class: "btn btn-primary btn-xs ml-auto",
                    disabled: !has_any_sprite,
                    onclick: on_add,
                    "+ Reference"
                }
            }
            if !has_any_sprite {
                p { class: "text-xs text-base-content/60 italic",
                    "参照可能な Sprite がありません。"
                }
            } else if snapshot.is_empty() {
                p { class: "text-xs text-base-content/60 italic", "Reference がありません。" }
            } else {
                div { class: "flex flex-col gap-2",
                    for (i, reference) in snapshot.iter().enumerate() {
                        ReferenceItem {
                            key: "{i}",
                            character: character.clone(),
                            list_index: i,
                            reference: reference.clone(),
                            references,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ReferenceItem(
    character: Character,
    list_index: usize,
    reference: SpriteReference,
    mut references: Signal<Vec<SpriteReference>>,
) -> Element {
    let current_group = character
        .sprite_groups
        .iter()
        .find(|g| g.number == reference.sprite_group_number)
        .cloned();

    let on_group = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        let mut updated = references();
        let Some(r) = updated.get_mut(list_index) else {
            return;
        };
        if r.sprite_group_number == v {
            return;
        }
        r.sprite_group_number = v;
        // 新グループに該当 sprite が無い可能性があるので 0 にリセット
        r.sprite_index = 0;
        references.set(updated);
    };

    let on_sprite = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        let mut updated = references();
        let Some(r) = updated.get_mut(list_index) else {
            return;
        };
        if r.sprite_index == v {
            return;
        }
        r.sprite_index = v;
        references.set(updated);
    };

    let on_placement = move |evt: Event<FormData>| {
        let new_placement = ReferencePlacement::from_str(&evt.value());
        let mut updated = references();
        let Some(r) = updated.get_mut(list_index) else {
            return;
        };
        if r.placement == new_placement {
            return;
        }
        r.placement = new_placement;
        references.set(updated);
    };

    let on_opacity = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<f32>() else {
            return;
        };
        let clamped = v.clamp(0.0, 1.0);
        let mut updated = references();
        let Some(r) = updated.get_mut(list_index) else {
            return;
        };
        if (r.opacity - clamped).abs() < f32::EPSILON {
            return;
        }
        r.opacity = clamped;
        references.set(updated);
    };

    let on_delete = move |_| {
        let mut updated = references();
        if list_index >= updated.len() {
            return;
        }
        updated.remove(list_index);
        references.set(updated);
    };

    let group_value = reference.sprite_group_number.to_string();
    let sprite_value = reference.sprite_index.to_string();
    let placement_value = reference.placement.as_str();

    rsx! {
        div { class: "p-2 rounded bg-base-100 space-y-1",
            div { class: "flex items-center gap-2",
                span { class: "badge badge-neutral badge-xs font-mono", "R{list_index}" }
                button {
                    class: "btn btn-ghost btn-xs ml-auto text-error",
                    title: "削除",
                    onclick: on_delete,
                    "✕"
                }
            }
            div { class: "grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 items-center",
                label { class: "text-xs", "Sprite Group" }
                select {
                    class: "select select-bordered select-xs w-full",
                    value: "{group_value}",
                    onchange: on_group,
                    for group in character.sprite_groups.iter() {
                        option { value: "{group.number}", "{group.name} (#{group.number})" }
                    }
                }
                label { class: "text-xs", "Sprite Index" }
                select {
                    class: "select select-bordered select-xs w-full",
                    value: "{sprite_value}",
                    onchange: on_sprite,
                    if let Some(group) = current_group.as_ref() {
                        if group.sprites.is_empty() {
                            option { value: "0", disabled: true, "（sprites 無し）" }
                        } else {
                            for sprite in group.sprites.iter() {
                                option { value: "{sprite.index}", "{sprite.index}: {sprite.path}" }
                            }
                        }
                    } else {
                        option { value: "{reference.sprite_index}",
                            "現在: {reference.sprite_index} (group 不明)"
                        }
                    }
                }
                label { class: "text-xs", "Placement" }
                select {
                    class: "select select-bordered select-xs w-full",
                    value: "{placement_value}",
                    onchange: on_placement,
                    option { value: "back", "Back (奥)" }
                    option { value: "front", "Front (手前)" }
                }
                label { class: "text-xs", "Opacity" }
                div { class: "flex items-center gap-2",
                    input {
                        r#type: "range",
                        class: "range range-xs flex-1",
                        min: "0",
                        max: "1",
                        step: "0.05",
                        value: "{reference.opacity}",
                        oninput: on_opacity,
                    }
                    span { class: "font-mono text-xs w-10 text-right", "{reference.opacity:.2}" }
                }
            }
        }
    }
}
