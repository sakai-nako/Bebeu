//! Project.hud.elements の編集フォーム (ADR-0029)。
//!
//! 要素ごとに 1 カード (kind / 位置 / サイズ / 枠 / 色 / 方向 / gauge_step) を並べ、
//! 末尾に追加ボタン。disk 保存に成功してから Signal を更新する pattern。
use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::project::{
    FillDirection, GaugeStep, HexColor, HudAnchor, HudElement, PlayerHpBarConfig, Project,
    ProjectRepository,
};
use crate::widgets::color_picker::ColorPickerPopover;

#[component]
pub fn EditHudLayout(project: Signal<Project>) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();
    let error = use_signal(|| None::<String>);

    let elements = project.read().hud.elements.clone();

    let add_element = {
        let repo = repo.clone();
        move |_| {
            let mut next = project.peek().clone();
            next.hud
                .elements
                .push(HudElement::PlayerHpBar(PlayerHpBarConfig::default()));
            commit(&repo, project, error, next);
        }
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
                        element: *element,
                        project,
                        error,
                    }
                }
            }
            div { class: "mt-2",
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
                }
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
            // 配置: anchor + offset
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
    let HudElement::PlayerHpBar(cfg) = element;
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
