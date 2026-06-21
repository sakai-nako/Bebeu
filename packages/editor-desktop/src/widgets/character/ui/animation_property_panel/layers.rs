use dioxus::prelude::*;

use super::{flip_to_value, parse_flip};
use crate::entities::character::{Animation, Character, Layer};
use crate::shared::UseHistory;

/// Frame.layers の `index` を配列順に揃える。add / delete / move のあとに必ず呼ぶ。
fn renumber_layers(layers: &mut [Layer]) {
    for (i, l) in layers.iter_mut().enumerate() {
        l.index = u32::try_from(i).unwrap_or(u32::MAX);
    }
}

#[component]
pub(super) fn LayerListSection(
    character: Character,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    layers: Vec<Layer>,
    mut selected_layer_index: Signal<Option<usize>>,
) -> Element {
    // 新規 Layer のデフォルト sprite 参照は、現在の Character の最初の SpriteGroup / Sprite から取る。
    // 事前にコピー by value しておけば、クロージャが character を partial move するのを防げる。
    let (default_group, default_sprite) = character.sprite_groups.first().map_or((0, 0), |g| {
        (g.number, g.sprites.first().map_or(0, |s| s.index))
    });

    let on_add = move |_| {
        history.record();
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let new_layer = Layer {
            index: u32::try_from(f.layers.len()).unwrap_or(u32::MAX),
            sprite_group_number: default_group,
            sprite_index: default_sprite,
            transparency: 1.0,
            flip: None,
            pivot_point_offset: None,
        };
        let new_idx = f.layers.len();
        f.layers.push(new_layer);
        renumber_layers(&mut f.layers);
        draft.set(updated);
        selected_layer_index.set(Some(new_idx));
    };

    let layers_len = layers.len();
    let selected = selected_layer_index();

    rsx! {
        div { class: "space-y-2",
            div { class: "flex items-center gap-2",
                h3 { class: "font-semibold", "Layers ({layers_len})" }
                div { class: "ml-auto" }
                button { class: "btn btn-primary btn-xs", onclick: on_add, "+ Layer" }
            }
            if layers.is_empty() {
                p { class: "text-xs text-base-content/60 italic", "Layer がありません。" }
            } else {
                div { class: "flex flex-col gap-1",
                    for (i, layer) in layers.iter().enumerate() {
                        LayerListItem {
                            key: "{layer.index}",
                            character: character.clone(),
                            layer: layer.clone(),
                            list_index: i,
                            list_len: layers_len,
                            is_selected: selected == Some(i),
                            draft,
                            history,
                            frame_index,
                            selected_layer_index,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LayerListItem(
    character: Character,
    layer: Layer,
    list_index: usize,
    list_len: usize,
    is_selected: bool,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    mut selected_layer_index: Signal<Option<usize>>,
) -> Element {
    let resolved = character.find_sprite(layer.sprite_group_number, layer.sprite_index);
    let label = resolved.as_ref().map_or_else(
        || format!("? #{}/{}", layer.sprite_group_number, layer.sprite_index),
        |(g, _s)| {
            format!(
                "{} #{}/{}",
                g.name, layer.sprite_group_number, layer.sprite_index
            )
        },
    );

    let row_class = if is_selected {
        "flex items-center gap-1 px-2 py-1 rounded text-xs bg-base-300 ring-1 ring-warning cursor-pointer"
    } else {
        "flex items-center gap-1 px-2 py-1 rounded text-xs hover:bg-base-300 cursor-pointer"
    };

    let on_click = move |_| selected_layer_index.set(Some(list_index));

    let on_up = move |evt: MouseEvent| {
        evt.stop_propagation();
        if list_index == 0 {
            return;
        }
        history.record();
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        f.layers.swap(list_index - 1, list_index);
        renumber_layers(&mut f.layers);
        draft.set(updated);
        selected_layer_index.set(Some(list_index - 1));
    };

    let on_down = move |evt: MouseEvent| {
        evt.stop_propagation();
        if list_index + 1 >= list_len {
            return;
        }
        history.record();
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        f.layers.swap(list_index, list_index + 1);
        renumber_layers(&mut f.layers);
        draft.set(updated);
        selected_layer_index.set(Some(list_index + 1));
    };

    let on_delete = move |evt: MouseEvent| {
        evt.stop_propagation();
        history.record();
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        if list_index >= f.layers.len() {
            return;
        }
        f.layers.remove(list_index);
        renumber_layers(&mut f.layers);
        draft.set(updated);
        selected_layer_index.set(None);
    };

    rsx! {
        div { class: "{row_class}", onclick: on_click,
            span { class: "badge badge-neutral badge-xs font-mono", "L{layer.index}" }
            span { class: "font-mono truncate flex-1", title: "{label}", "{label}" }
            span { class: "font-mono text-base-content/60", "{layer.transparency:.2}" }
            button {
                class: "btn btn-ghost btn-xs",
                disabled: list_index == 0,
                title: "上へ",
                onclick: on_up,
                "↑"
            }
            button {
                class: "btn btn-ghost btn-xs",
                disabled: list_index + 1 >= list_len,
                title: "下へ",
                onclick: on_down,
                "↓"
            }
            button {
                class: "btn btn-ghost btn-xs text-error",
                title: "削除",
                onclick: on_delete,
                "✕"
            }
        }
    }
}

#[component]
pub(super) fn SelectedLayerEditor(
    character: Character,
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    frame_index: usize,
    layer_index: usize,
    layer: Layer,
    mut selected_layer_index: Signal<Option<usize>>,
) -> Element {
    // 選択中の SpriteGroup（draft 内の参照解決のため Character.sprite_groups から探す）
    let current_group = character
        .sprite_groups
        .iter()
        .find(|g| g.number == layer.sprite_group_number)
        .cloned();

    let on_group = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let Some(l) = f.layers.get_mut(layer_index) else {
            return;
        };
        if l.sprite_group_number == v {
            return;
        }
        l.sprite_group_number = v;
        // sprite_index は新グループで存在しないかもしれないので 0 にリセット
        l.sprite_index = 0;
        history.record();
        draft.set(updated);
    };

    let on_sprite = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let Some(l) = f.layers.get_mut(layer_index) else {
            return;
        };
        if l.sprite_index == v {
            return;
        }
        l.sprite_index = v;
        history.record();
        draft.set(updated);
    };

    let on_transparency = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<f32>() else {
            return;
        };
        let clamped = v.clamp(0.0, 1.0);
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let Some(l) = f.layers.get_mut(layer_index) else {
            return;
        };
        if (l.transparency - clamped).abs() < f32::EPSILON {
            return;
        }
        l.transparency = clamped;
        history.record();
        draft.set(updated);
    };

    let on_flip = move |evt: Event<FormData>| {
        let new_flip = parse_flip(&evt.value());
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let Some(l) = f.layers.get_mut(layer_index) else {
            return;
        };
        if l.flip == new_flip {
            return;
        }
        l.flip = new_flip;
        history.record();
        draft.set(updated);
    };

    let off = layer.pivot_point_offset.unwrap_or([0, 0]);
    let mut on_offset = move |axis: usize, evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<i32>() else {
            return;
        };
        let mut updated = draft();
        let Some(f) = updated.frames.get_mut(frame_index) else {
            return;
        };
        let Some(l) = f.layers.get_mut(layer_index) else {
            return;
        };
        let mut current = l.pivot_point_offset.unwrap_or([0, 0]);
        if current[axis] == v {
            return;
        }
        current[axis] = v;
        l.pivot_point_offset = if current == [0, 0] {
            None
        } else {
            Some(current)
        };
        history.record();
        draft.set(updated);
    };

    let on_clear_selection = move |_| selected_layer_index.set(None);

    let flip_value = flip_to_value(layer.flip);
    let group_value = layer.sprite_group_number.to_string();
    let sprite_value = layer.sprite_index.to_string();

    rsx! {
        div { class: "space-y-2",
            div { class: "flex items-center gap-2",
                h3 { class: "font-semibold", "Layer #{layer.index}" }
                button {
                    class: "btn btn-ghost btn-xs ml-auto",
                    onclick: on_clear_selection,
                    "選択解除"
                }
            }
            div { class: "grid grid-cols-[auto_1fr] gap-x-2 gap-y-2 items-center",
                label { class: "text-xs", "Sprite Group" }
                select {
                    class: "select select-bordered select-sm w-full",
                    value: "{group_value}",
                    onchange: on_group,
                    for group in character.sprite_groups.iter() {
                        // option 側にも `selected:` を必ず付ける。Dioxus desktop の webview では
                        // `<select value="...">` だけだと controlled state がブラウザ DOM と
                        // ずれ、ユーザの選択が revert されて onchange が「現在値と同じ値」で
                        // 発火 → on_group の早期 return で握り潰される (= 反映されない / dirty に
                        // ならない) という症状になる。role_section が正常動作しているのと
                        // 同じ理由で `selected:` を併用する。
                        option {
                            value: "{group.number}",
                            selected: group.number == layer.sprite_group_number,
                            "{group.name} (#{group.number})"
                        }
                    }
                }
                label { class: "text-xs", "Sprite Index" }
                select {
                    class: "select select-bordered select-sm w-full",
                    value: "{sprite_value}",
                    onchange: on_sprite,
                    if let Some(group) = current_group.as_ref() {
                        if group.sprites.is_empty() {
                            option { value: "0", disabled: true, "（sprites 無し）" }
                        } else {
                            for sprite in group.sprites.iter() {
                                option {
                                    value: "{sprite.index}",
                                    selected: sprite.index == layer.sprite_index,
                                    "{sprite.index}: {sprite.path}"
                                }
                            }
                        }
                    } else {
                        option {
                            value: "{layer.sprite_index}",
                            selected: true,
                            "現在: {layer.sprite_index} (group 不明)"
                        }
                    }
                }
                label { class: "text-xs", "Transparency" }
                div { class: "flex items-center gap-2",
                    input {
                        r#type: "range",
                        class: "range range-xs flex-1",
                        min: "0",
                        max: "1",
                        step: "0.05",
                        value: "{layer.transparency}",
                        oninput: on_transparency,
                    }
                    span { class: "font-mono text-xs w-10 text-right", "{layer.transparency:.2}" }
                }
                label { class: "text-xs", "Flip" }
                select {
                    class: "select select-bordered select-sm w-32",
                    value: "{flip_value}",
                    onchange: on_flip,
                    option { value: "none", "None" }
                    option { value: "horizontal", "Horizontal" }
                    option { value: "vertical", "Vertical" }
                    option { value: "both", "Both" }
                }
                label { class: "text-xs", "Pivot Offset" }
                div { class: "flex items-center gap-1",
                    span { class: "text-xs", "x" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-16",
                        value: "{off[0]}",
                        onchange: move |evt| on_offset(0, evt),
                    }
                    span { class: "text-xs", "y" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-16",
                        value: "{off[1]}",
                        onchange: move |evt| on_offset(1, evt),
                    }
                }
            }
        }
    }
}
