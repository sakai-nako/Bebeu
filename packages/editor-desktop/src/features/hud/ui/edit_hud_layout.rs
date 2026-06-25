//! Project.hud.elements の編集フォーム (ADR-0029)。
//!
//! 要素ごとに 1 カード (kind / 位置 / サイズ / 枠 / 色 / 方向 / gauge_step) を並べ、
//! 末尾に追加ボタン。disk 保存に成功してから Signal を更新する pattern。
use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::project::{
    EnemyHpBarConfig, EnemyOverheadHpBarConfig, EnemyTarget, FillDirection, GaugeStep, HexColor,
    HudAnchor, HudElement, OverheadVerticalAnchor, PlayerHpBarConfig, PlayerHpRingConfig, PlayerId,
    Project, ProjectRepository, RingDirection,
};
use crate::widgets::color_picker::ColorPickerPopover;

#[component]
pub fn EditHudLayout(project: Signal<Project>) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();
    let error = use_signal(|| None::<String>);

    // 追加対象の kind。dropdown の現在値。default は先頭 (= Player HP bar)。
    let mut selected_kind = use_signal(|| HudElement::all_kinds()[0].value.to_string());

    let elements = project.read().hud.elements.clone();

    let add_element = {
        let repo = repo.clone();
        move |_| {
            let kind = selected_kind.peek().clone();
            let Some(element) = HudElement::default_for_kind(&kind) else {
                return;
            };
            let mut next = project.peek().clone();
            next.hud.elements.push(element);
            commit(&repo, project, error, next);
        }
    };
    let on_kind_change = move |evt: Event<FormData>| {
        selected_kind.set(evt.value());
    };

    rsx! {
        fieldset { class: "fieldset",
            legend { class: "fieldset-legend", "HUD レイアウト" }
            p { class: "text-xs text-base-content/60 mb-2",
                "Gameplay 中の HUD 要素。anchor + offset (viewport ピクセル単位) で配置し、"
                "size は外形 (枠はその内側に食い込む)。"
            }
            div { class: "flex flex-col gap-3",
                for (index, element) in elements.iter().enumerate() {
                    HudElementRow {
                        key: "{index}",
                        index,
                        element: element.clone(),
                        project,
                        error,
                    }
                }
            }
            div { class: "mt-2 flex items-end gap-2",
                label { class: "form-control",
                    span { class: "label-text text-xs", "種類" }
                    select {
                        class: "select select-sm select-bordered",
                        value: "{selected_kind}",
                        onchange: on_kind_change,
                        for opt in HudElement::all_kinds() {
                            option { value: opt.value, "{opt.label}" }
                        }
                    }
                }
                button {
                    r#type: "button",
                    class: "btn btn-sm btn-outline",
                    onclick: add_element,
                    "+ 要素を追加"
                }
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        }
    }
}

#[component]
fn HudElementRow(
    index: usize,
    element: HudElement,
    project: Signal<Project>,
    error: Signal<Option<String>>,
) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();

    let on_delete = {
        let repo = repo.clone();
        move |_| {
            let mut next = project.peek().clone();
            if index < next.hud.elements.len() {
                next.hud.elements.remove(index);
                commit(&repo, project, error, next);
            }
        }
    };

    rsx! {
        div { class: "rounded border border-base-300 p-3 space-y-2",
            div { class: "flex items-center gap-2 flex-wrap",
                span { class: "font-semibold text-sm", "{element.kind_label()}" }
                button {
                    r#type: "button",
                    class: "btn btn-sm btn-ghost text-error ml-auto",
                    onclick: on_delete,
                    "削除"
                }
            }
            match element {
                HudElement::PlayerHpBar(cfg) => rsx! {
                    PlayerHpBarEditor { index, cfg, project, error }
                },
                HudElement::PlayerHpRing(cfg) => rsx! {
                    PlayerHpRingEditor { index, cfg, project, error }
                },
                HudElement::EnemyHpBar(cfg) => rsx! {
                    EnemyHpBarEditor { index, cfg, project, error }
                },
                HudElement::EnemyOverheadHpBar(cfg) => rsx! {
                    EnemyOverheadHpBarEditor { index, cfg, project, error }
                },
            }
        }
    }
}

#[component]
fn PlayerHpBarEditor(
    index: usize,
    cfg: PlayerHpBarConfig,
    project: Signal<Project>,
    error: Signal<Option<String>>,
) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();

    let on_target_change = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(target) = PlayerId::parse(&evt.value()) {
                update_player_hp_bar(&repo, project, error, index, |c| c.target = target);
            }
        }
    };
    let on_anchor_change = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(anchor) = HudAnchor::parse(&evt.value()) {
                update_player_hp_bar(&repo, project, error, index, |c| c.anchor = anchor);
            }
        }
    };
    let on_offset_x = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_bar(&repo, project, error, index, |c| c.offset.x = v);
            }
        }
    };
    let on_offset_y = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_bar(&repo, project, error, index, |c| c.offset.y = v);
            }
        }
    };
    let on_size_w = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_bar(&repo, project, error, index, |c| c.size.w = v);
            }
        }
    };
    let on_size_h = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_bar(&repo, project, error, index, |c| c.size.h = v);
            }
        }
    };
    let on_frame_thickness = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_bar(&repo, project, error, index, |c| c.frame.thickness = v);
            }
        }
    };
    let on_frame_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_player_hp_bar(&repo, project, error, index, move |cfg| cfg.frame.color = c);
        }
    };
    let on_bg_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_player_hp_bar(&repo, project, error, index, move |cfg| cfg.bg_color = c);
        }
    };
    let on_fg_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_player_hp_bar(&repo, project, error, index, move |cfg| cfg.fg_color = c);
        }
    };
    let on_fill_direction = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(d) = FillDirection::parse(&evt.value()) {
                update_player_hp_bar(&repo, project, error, index, |c| c.fill_direction = d);
            }
        }
    };
    let on_gauge_kind = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(next) = GaugeStep::with_value(&evt.value(), cfg.gauge_step.amount()) {
                update_player_hp_bar(&repo, project, error, index, move |c| c.gauge_step = next);
            }
        }
    };
    let on_gauge_amount = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<u32>() {
                let next = cfg.gauge_step.with_amount(v);
                update_player_hp_bar(&repo, project, error, index, move |c| c.gauge_step = next);
            }
        }
    };
    let on_gauge_gap = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_bar(&repo, project, error, index, |c| c.gauge_gap = v);
            }
        }
    };

    rsx! {
        div { class: "space-y-2",
            // 対象 Player + 配置 (anchor + offset)
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Target" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.target.value(),
                        onchange: on_target_change,
                        for p in PlayerId::ALL {
                            option { value: p.value(), "{p.label()}" }
                        }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Anchor" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.anchor.value(),
                        onchange: on_anchor_change,
                        for a in HudAnchor::ALL {
                            option { value: a.value(), "{a.label()}" }
                        }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Offset X" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.offset.x}",
                        onchange: on_offset_x,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Offset Y" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.offset.y}",
                        onchange: on_offset_y,
                    }
                }
            }
            // サイズ: size
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Size W" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.size.w}",
                        onchange: on_size_w,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Size H" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.size.h}",
                        onchange: on_size_h,
                    }
                }
            }
            // 枠: frame.thickness + frame.color
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Frame thickness" }
                    input {
                        r#type: "number",
                        step: "0.5",
                        min: "0",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.frame.thickness}",
                        onchange: on_frame_thickness,
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "Frame color" }
                    ColorPickerPopover {
                        value: cfg.frame.color,
                        on_change: EventHandler::new(on_frame_color),
                    }
                }
            }
            // 内側 bg/fg 色 + alpha
            div { class: "flex items-end gap-2 flex-wrap",
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "BG color" }
                    ColorPickerPopover {
                        value: cfg.bg_color,
                        on_change: EventHandler::new(on_bg_color),
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "FG color" }
                    ColorPickerPopover {
                        value: cfg.fg_color,
                        on_change: EventHandler::new(on_fg_color),
                    }
                }
            }
            // 方向 + gauge_step + gap
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Fill direction" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.fill_direction.value(),
                        onchange: on_fill_direction,
                        for d in FillDirection::ALL {
                            option { value: d.value(), "{d.label()}" }
                        }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Gauge step" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.gauge_step.value(),
                        onchange: on_gauge_kind,
                        option { value: "fixed_count", "Fixed count (本数固定)" }
                        option { value: "per_unit", "Per unit (1 本 = N HP)" }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Amount" }
                    input {
                        r#type: "number",
                        min: "1",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.gauge_step.amount()}",
                        onchange: on_gauge_amount,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Gauge gap" }
                    input {
                        r#type: "number",
                        step: "0.5",
                        min: "0",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.gauge_gap}",
                        onchange: on_gauge_gap,
                    }
                }
            }
        }
    }
}

fn update_player_hp_bar(
    repo: &Arc<dyn ProjectRepository>,
    project: Signal<Project>,
    error: Signal<Option<String>>,
    index: usize,
    mutate: impl FnOnce(&mut PlayerHpBarConfig),
) {
    let mut next = project.peek().clone();
    let Some(element) = next.hud.elements.get_mut(index) else {
        return;
    };
    let HudElement::PlayerHpBar(cfg) = element else {
        return;
    };
    mutate(cfg);
    commit(repo, project, error, next);
}

#[component]
fn PlayerHpRingEditor(
    index: usize,
    cfg: PlayerHpRingConfig,
    project: Signal<Project>,
    error: Signal<Option<String>>,
) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();

    let on_target_change = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(target) = PlayerId::parse(&evt.value()) {
                update_player_hp_ring(&repo, project, error, index, |c| c.target = target);
            }
        }
    };
    let on_anchor_change = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(anchor) = HudAnchor::parse(&evt.value()) {
                update_player_hp_ring(&repo, project, error, index, |c| c.anchor = anchor);
            }
        }
    };
    let on_offset_x = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.offset.x = v);
            }
        }
    };
    let on_offset_y = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.offset.y = v);
            }
        }
    };
    let on_size_w = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.size.w = v);
            }
        }
    };
    let on_size_h = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.size.h = v);
            }
        }
    };
    let on_frame_thickness = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.frame.thickness = v);
            }
        }
    };
    let on_frame_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_player_hp_ring(&repo, project, error, index, move |cfg| cfg.frame.color = c);
        }
    };
    let on_bg_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_player_hp_ring(&repo, project, error, index, move |cfg| cfg.bg_color = c);
        }
    };
    let on_fg_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_player_hp_ring(&repo, project, error, index, move |cfg| cfg.fg_color = c);
        }
    };
    let on_start_angle = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.start_angle = v);
            }
        }
    };
    let on_sweep_extent = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.sweep_extent = v);
            }
        }
    };
    let on_ring_thickness = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.ring_thickness = v);
            }
        }
    };
    let on_direction = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(d) = RingDirection::parse(&evt.value()) {
                update_player_hp_ring(&repo, project, error, index, |c| c.direction = d);
            }
        }
    };
    let on_gauge_kind = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(next) = GaugeStep::with_value(&evt.value(), cfg.gauge_step.amount()) {
                update_player_hp_ring(&repo, project, error, index, move |c| {
                    c.gauge_step = next;
                });
            }
        }
    };
    let on_gauge_amount = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<u32>() {
                let next = cfg.gauge_step.with_amount(v);
                update_player_hp_ring(&repo, project, error, index, move |c| {
                    c.gauge_step = next;
                });
            }
        }
    };
    let on_gauge_gap = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_player_hp_ring(&repo, project, error, index, |c| c.gauge_gap = v);
            }
        }
    };

    rsx! {
        div { class: "space-y-2",
            // 対象 Player + 配置 (anchor + offset)
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Target" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.target.value(),
                        onchange: on_target_change,
                        for p in PlayerId::ALL {
                            option { value: p.value(), "{p.label()}" }
                        }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Anchor" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.anchor.value(),
                        onchange: on_anchor_change,
                        for a in HudAnchor::ALL {
                            option { value: a.value(), "{a.label()}" }
                        }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Offset X" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.offset.x}",
                        onchange: on_offset_x,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Offset Y" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.offset.y}",
                        onchange: on_offset_y,
                    }
                }
            }
            // サイズ: size (外接 bbox)
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Size W" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.size.w}",
                        onchange: on_size_w,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Size H" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.size.h}",
                        onchange: on_size_h,
                    }
                }
            }
            // 枠: frame.thickness + frame.color
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Frame thickness" }
                    input {
                        r#type: "number",
                        step: "0.5",
                        min: "0",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.frame.thickness}",
                        onchange: on_frame_thickness,
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "Frame color" }
                    ColorPickerPopover {
                        value: cfg.frame.color,
                        on_change: EventHandler::new(on_frame_color),
                    }
                }
            }
            // 内側 bg/fg 色
            div { class: "flex items-end gap-2 flex-wrap",
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "BG color" }
                    ColorPickerPopover {
                        value: cfg.bg_color,
                        on_change: EventHandler::new(on_bg_color),
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "FG color" }
                    ColorPickerPopover {
                        value: cfg.fg_color,
                        on_change: EventHandler::new(on_fg_color),
                    }
                }
            }
            // 角度系: start / sweep / thickness / direction
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Start angle (°)" }
                    input {
                        r#type: "number",
                        step: "1",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.start_angle}",
                        onchange: on_start_angle,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Sweep (°)" }
                    input {
                        r#type: "number",
                        step: "1",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.sweep_extent}",
                        onchange: on_sweep_extent,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Ring thickness" }
                    input {
                        r#type: "number",
                        step: "0.5",
                        min: "0",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.ring_thickness}",
                        onchange: on_ring_thickness,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Direction" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.direction.value(),
                        onchange: on_direction,
                        for d in RingDirection::ALL {
                            option { value: d.value(), "{d.label()}" }
                        }
                    }
                }
            }
            // gauge_step + gap (度単位)
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Gauge step" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.gauge_step.value(),
                        onchange: on_gauge_kind,
                        option { value: "fixed_count", "Fixed count (本数固定)" }
                        option { value: "per_unit", "Per unit (1 本 = N HP)" }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Amount" }
                    input {
                        r#type: "number",
                        min: "1",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.gauge_step.amount()}",
                        onchange: on_gauge_amount,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Gauge gap (°)" }
                    input {
                        r#type: "number",
                        step: "0.5",
                        min: "0",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.gauge_gap}",
                        onchange: on_gauge_gap,
                    }
                }
            }
        }
    }
}

fn update_player_hp_ring(
    repo: &Arc<dyn ProjectRepository>,
    project: Signal<Project>,
    error: Signal<Option<String>>,
    index: usize,
    mutate: impl FnOnce(&mut PlayerHpRingConfig),
) {
    let mut next = project.peek().clone();
    let Some(element) = next.hud.elements.get_mut(index) else {
        return;
    };
    let HudElement::PlayerHpRing(cfg) = element else {
        return;
    };
    mutate(cfg);
    commit(repo, project, error, next);
}

fn commit(
    repo: &Arc<dyn ProjectRepository>,
    mut project: Signal<Project>,
    mut error: Signal<Option<String>>,
    next: Project,
) {
    match repo.update(&next) {
        Ok(()) => {
            project.set(next);
            error.set(None);
        }
        Err(e) => error.set(Some(e.to_string())),
    }
}

#[component]
fn EnemyHpBarEditor(
    index: usize,
    cfg: EnemyHpBarConfig,
    project: Signal<Project>,
    error: Signal<Option<String>>,
) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();

    // target variant 切替: variant 名を変えても適切な default 値を入れて切替える。
    let on_target_kind = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            let next_target = match evt.value().as_str() {
                "last_engaged_by" => EnemyTarget::LastEngagedBy(PlayerId::P1),
                "tag" => EnemyTarget::Tag(String::new()),
                "nth_enemy" => EnemyTarget::NthEnemy(0),
                _ => return,
            };
            update_enemy_hp_bar(&repo, project, error, index, move |c| {
                c.target = next_target;
            });
        }
    };
    let on_target_player = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(pid) = PlayerId::parse(&evt.value()) {
                update_enemy_hp_bar(&repo, project, error, index, move |c| {
                    c.target = EnemyTarget::LastEngagedBy(pid);
                });
            }
        }
    };
    let on_target_tag = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            let s = evt.value();
            update_enemy_hp_bar(&repo, project, error, index, move |c| {
                c.target = EnemyTarget::Tag(s);
            });
        }
    };
    let on_target_index = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<usize>() {
                update_enemy_hp_bar(&repo, project, error, index, move |c| {
                    c.target = EnemyTarget::NthEnemy(v);
                });
            }
        }
    };
    let on_anchor_change = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(anchor) = HudAnchor::parse(&evt.value()) {
                update_enemy_hp_bar(&repo, project, error, index, |c| c.anchor = anchor);
            }
        }
    };
    let on_offset_x = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_hp_bar(&repo, project, error, index, |c| c.offset.x = v);
            }
        }
    };
    let on_offset_y = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_hp_bar(&repo, project, error, index, |c| c.offset.y = v);
            }
        }
    };
    let on_size_w = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_hp_bar(&repo, project, error, index, |c| c.size.w = v);
            }
        }
    };
    let on_size_h = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_hp_bar(&repo, project, error, index, |c| c.size.h = v);
            }
        }
    };
    let on_frame_thickness = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_hp_bar(&repo, project, error, index, |c| c.frame.thickness = v);
            }
        }
    };
    let on_frame_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_enemy_hp_bar(&repo, project, error, index, move |cfg| cfg.frame.color = c);
        }
    };
    let on_bg_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_enemy_hp_bar(&repo, project, error, index, move |cfg| cfg.bg_color = c);
        }
    };
    let on_fg_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_enemy_hp_bar(&repo, project, error, index, move |cfg| cfg.fg_color = c);
        }
    };
    let on_fill_direction = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(d) = FillDirection::parse(&evt.value()) {
                update_enemy_hp_bar(&repo, project, error, index, |c| c.fill_direction = d);
            }
        }
    };

    // target の variant ごとに 1 つ追加 input を出す。
    let target_value_input = match &cfg.target {
        EnemyTarget::LastEngagedBy(pid) => rsx! {
            label { class: "form-control",
                span { class: "label-text text-xs", "Player" }
                select {
                    class: "select select-sm select-bordered",
                    value: pid.value(),
                    onchange: on_target_player,
                    for p in PlayerId::ALL {
                        option { value: p.value(), "{p.label()}" }
                    }
                }
            }
        },
        EnemyTarget::Tag(s) => rsx! {
            label { class: "form-control",
                span { class: "label-text text-xs", "Tag" }
                input {
                    r#type: "text",
                    class: "input input-sm input-bordered w-40",
                    value: "{s}",
                    onchange: on_target_tag,
                }
            }
        },
        EnemyTarget::NthEnemy(n) => rsx! {
            label { class: "form-control",
                span { class: "label-text text-xs", "Index" }
                input {
                    r#type: "number",
                    min: "0",
                    class: "input input-sm input-bordered w-24",
                    value: "{n}",
                    onchange: on_target_index,
                }
            }
        },
    };

    rsx! {
        div { class: "space-y-2",
            // Target variant + variant ごとの追加 input
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Target kind" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.target.value(),
                        onchange: on_target_kind,
                        for (v, label) in EnemyTarget::ALL_VARIANTS {
                            option { value: *v, "{label}" }
                        }
                    }
                }
                {target_value_input}
            }
            // Anchor + offset
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Anchor" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.anchor.value(),
                        onchange: on_anchor_change,
                        for a in HudAnchor::ALL {
                            option { value: a.value(), "{a.label()}" }
                        }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Offset X" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.offset.x}",
                        onchange: on_offset_x,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Offset Y" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.offset.y}",
                        onchange: on_offset_y,
                    }
                }
            }
            // サイズ
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Size W" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.size.w}",
                        onchange: on_size_w,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Size H" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.size.h}",
                        onchange: on_size_h,
                    }
                }
            }
            // 枠
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Frame thickness" }
                    input {
                        r#type: "number",
                        step: "0.5",
                        min: "0",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.frame.thickness}",
                        onchange: on_frame_thickness,
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "Frame color" }
                    ColorPickerPopover {
                        value: cfg.frame.color,
                        on_change: EventHandler::new(on_frame_color),
                    }
                }
            }
            // bg/fg
            div { class: "flex items-end gap-2 flex-wrap",
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "BG color" }
                    ColorPickerPopover {
                        value: cfg.bg_color,
                        on_change: EventHandler::new(on_bg_color),
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "FG color" }
                    ColorPickerPopover {
                        value: cfg.fg_color,
                        on_change: EventHandler::new(on_fg_color),
                    }
                }
            }
            // Fill direction (gauge_step は Phase 2 では engine 側で FixedCount(1) 強制)
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Fill direction" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.fill_direction.value(),
                        onchange: on_fill_direction,
                        for d in FillDirection::ALL {
                            option { value: d.value(), "{d.label()}" }
                        }
                    }
                }
                p { class: "text-xs text-base-content/60",
                    "(Phase 2: enemy bar は単一 gauge 固定)"
                }
            }
        }
    }
}

fn update_enemy_hp_bar(
    repo: &Arc<dyn ProjectRepository>,
    project: Signal<Project>,
    error: Signal<Option<String>>,
    index: usize,
    mutate: impl FnOnce(&mut EnemyHpBarConfig),
) {
    let mut next = project.peek().clone();
    let Some(element) = next.hud.elements.get_mut(index) else {
        return;
    };
    let HudElement::EnemyHpBar(cfg) = element else {
        return;
    };
    mutate(cfg);
    commit(repo, project, error, next);
}

#[component]
fn EnemyOverheadHpBarEditor(
    index: usize,
    cfg: EnemyOverheadHpBarConfig,
    project: Signal<Project>,
    error: Signal<Option<String>>,
) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();

    let on_tag_filter = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            let s = evt.value();
            update_enemy_overhead_hp_bar(&repo, project, error, index, move |c| {
                // 空文字は None として扱う (= 全 enemy)。
                c.tag_filter = if s.trim().is_empty() { None } else { Some(s) };
            });
        }
    };
    let on_size_w = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_overhead_hp_bar(&repo, project, error, index, |c| c.size.w = v);
            }
        }
    };
    let on_size_h = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_overhead_hp_bar(&repo, project, error, index, |c| c.size.h = v);
            }
        }
    };
    let on_vertical_anchor = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(a) = OverheadVerticalAnchor::parse(&evt.value()) {
                update_enemy_overhead_hp_bar(&repo, project, error, index, |c| {
                    c.vertical_anchor = a;
                });
            }
        }
    };
    let on_offset_y = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_overhead_hp_bar(&repo, project, error, index, |c| {
                    c.offset_y = v;
                });
            }
        }
    };
    let on_frame_thickness = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Ok(v) = evt.value().trim().parse::<f32>() {
                update_enemy_overhead_hp_bar(&repo, project, error, index, |c| {
                    c.frame.thickness = v;
                });
            }
        }
    };
    let on_frame_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_enemy_overhead_hp_bar(&repo, project, error, index, move |cfg| {
                cfg.frame.color = c;
            });
        }
    };
    let on_bg_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_enemy_overhead_hp_bar(&repo, project, error, index, move |cfg| cfg.bg_color = c);
        }
    };
    let on_fg_color = {
        let repo = repo.clone();
        move |c: HexColor| {
            update_enemy_overhead_hp_bar(&repo, project, error, index, move |cfg| cfg.fg_color = c);
        }
    };
    let on_fill_direction = {
        let repo = repo.clone();
        move |evt: Event<FormData>| {
            if let Some(d) = FillDirection::parse(&evt.value()) {
                update_enemy_overhead_hp_bar(&repo, project, error, index, |c| {
                    c.fill_direction = d;
                });
            }
        }
    };

    let tag_value = cfg.tag_filter.clone().unwrap_or_default();

    rsx! {
        div { class: "space-y-2",
            // Tag filter (空 = 全 enemy)
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Tag filter (空 = 全 enemy)" }
                    input {
                        r#type: "text",
                        class: "input input-sm input-bordered w-40",
                        value: "{tag_value}",
                        onchange: on_tag_filter,
                    }
                }
            }
            // Vertical anchor + offset Y
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Vertical anchor" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.vertical_anchor.value(),
                        onchange: on_vertical_anchor,
                        for a in OverheadVerticalAnchor::ALL {
                            option { value: a.value(), "{a.label()}" }
                        }
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Offset Y (+ 上 / − 下)" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.offset_y}",
                        onchange: on_offset_y,
                    }
                }
            }
            // サイズ
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Size W" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.size.w}",
                        onchange: on_size_w,
                    }
                }
                label { class: "form-control",
                    span { class: "label-text text-xs", "Size H" }
                    input {
                        r#type: "number",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.size.h}",
                        onchange: on_size_h,
                    }
                }
            }
            // 枠
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Frame thickness" }
                    input {
                        r#type: "number",
                        step: "0.5",
                        min: "0",
                        class: "input input-sm input-bordered w-24",
                        value: "{cfg.frame.thickness}",
                        onchange: on_frame_thickness,
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "Frame color" }
                    ColorPickerPopover {
                        value: cfg.frame.color,
                        on_change: EventHandler::new(on_frame_color),
                    }
                }
            }
            // bg/fg
            div { class: "flex items-end gap-2 flex-wrap",
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "BG color" }
                    ColorPickerPopover {
                        value: cfg.bg_color,
                        on_change: EventHandler::new(on_bg_color),
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "label-text text-xs", "FG color" }
                    ColorPickerPopover {
                        value: cfg.fg_color,
                        on_change: EventHandler::new(on_fg_color),
                    }
                }
            }
            // Fill direction
            div { class: "flex items-end gap-2 flex-wrap",
                label { class: "form-control",
                    span { class: "label-text text-xs", "Fill direction" }
                    select {
                        class: "select select-sm select-bordered",
                        value: cfg.fill_direction.value(),
                        onchange: on_fill_direction,
                        for d in FillDirection::ALL {
                            option { value: d.value(), "{d.label()}" }
                        }
                    }
                }
                p { class: "text-xs text-base-content/60",
                    "(World-anchored: 各 enemy の頭上に追従、単一 gauge 固定)"
                }
            }
        }
    }
}

fn update_enemy_overhead_hp_bar(
    repo: &Arc<dyn ProjectRepository>,
    project: Signal<Project>,
    error: Signal<Option<String>>,
    index: usize,
    mutate: impl FnOnce(&mut EnemyOverheadHpBarConfig),
) {
    let mut next = project.peek().clone();
    let Some(element) = next.hud.elements.get_mut(index) else {
        return;
    };
    let HudElement::EnemyOverheadHpBar(cfg) = element else {
        return;
    };
    mutate(cfg);
    commit(repo, project, error, next);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_value_and_parse_round_trip() {
        for anchor in HudAnchor::ALL {
            let s = anchor.value();
            assert_eq!(HudAnchor::parse(s), Some(*anchor));
        }
    }

    #[test]
    fn fill_direction_value_and_parse_round_trip() {
        for d in FillDirection::ALL {
            let s = d.value();
            assert_eq!(FillDirection::parse(s), Some(*d));
        }
    }

    #[test]
    fn gauge_step_with_value_preserves_amount() {
        assert_eq!(
            GaugeStep::with_value("per_unit", 100),
            Some(GaugeStep::PerUnit(100))
        );
        assert_eq!(
            GaugeStep::with_value("fixed_count", 3),
            Some(GaugeStep::FixedCount(3))
        );
    }
}
