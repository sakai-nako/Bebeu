//! Character.ai (AI Brain config) 編集 collapsible (ADR-0035 Phase 5)。
//!
//! `Properties` の下に置き、kind dropdown + selector dropdown + field grid で
//! `AiConfig::{Melee, Ally, Bot}` を編集する。`Physics` セクションと同形のレイアウト。

use dioxus::prelude::*;

use crate::entities::character::{AiKind, Character};
use crate::features::character::{
    AiF32Field, AiU32Field, EditAiF32Inline, EditAiKindInline, EditAiSelectorInline,
    EditAiU32Inline,
};

#[component]
pub fn AiSection(character: Character) -> Element {
    let kind = character.ai.as_ref().map(|c| c.kind());

    let f32_fields = [
        AiF32Field::ChaseEnterRangePx,
        AiF32Field::ChaseExitRangePx,
        AiF32Field::AttackEnterRangePx,
        AiF32Field::AttackExitRangePx,
        AiF32Field::FollowDistanceMinPx,
        AiF32Field::FollowDistanceMaxPx,
    ];
    let u32_fields = [
        AiU32Field::AttackCooldownTicks,
        AiU32Field::DecisionIntervalTicks,
        AiU32Field::MinDwellTicks,
    ];

    rsx! {
        div { class: "max-w-md w-full",
            div { class: "flex items-center justify-between mb-2",
                h2 { class: "text-xl font-semibold", "AI Brain" }
            }
            div { class: "collapse collapse-arrow bg-base-200",
                input { r#type: "checkbox" }
                div { class: "collapse-title text-sm text-base-content/70",
                    "Brain 種別 / target 選定 / Idle・Chase・Attack のパラメータ"
                }
                div { class: "collapse-content",
                    dl { class: "grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 items-center",
                        dt {
                            class: "font-semibold text-base-content/70",
                            title: "AI Brain の種別。none で AI を attach しない",
                            "Kind"
                        }
                        dd {
                            EditAiKindInline { character: character.clone() }
                        }
                        if kind.is_some() {
                            dt {
                                class: "font-semibold text-base-content/70",
                                title: "target を選ぶ戦略 (ADR-0039)。Random / WeightedByThreat は engine 側 stub",
                                "Target Selector"
                            }
                            dd {
                                EditAiSelectorInline { character: character.clone() }
                            }
                            for f in f32_fields.iter().copied() {
                                if f.is_applicable(kind.expect("checked above")) {
                                    dt {
                                        class: "font-semibold text-base-content/70",
                                        title: "{f.tooltip()}",
                                        "{f.label()}"
                                    }
                                    dd {
                                        EditAiF32Inline { character: character.clone(), field: f }
                                    }
                                }
                            }
                            for f in u32_fields.iter().copied() {
                                dt {
                                    class: "font-semibold text-base-content/70",
                                    title: "{f.tooltip()}",
                                    "{f.label()}"
                                }
                                dd {
                                    EditAiU32Inline { character: character.clone(), field: f }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
