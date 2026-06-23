use dioxus::prelude::*;

use crate::entities::character::Character;
use crate::features::character::{
    EditPhysicsF32Inline, EditPhysicsU32Inline, PhysicsF32Field, PhysicsU32Field,
};

/// Character の Properties エリアに並べる Physics 編集セクション。
///
/// 12 個のフィールド (重力 / ジャンプ初速 / Knockback ゲージ / バウンス / 摩擦 / timer /
/// コンボ上限) を単一の collapsible にまとめ、`EditPhysicsF32Inline` /
/// `EditPhysicsU32Inline` を流し込む。各フィールドの label / tooltip は enum 側で
/// 定義されているので、本コンポーネントはレイアウトと並び順のみ担当する。
#[component]
pub fn PhysicsSection(character: Character) -> Element {
    let f32_fields = [
        PhysicsF32Field::Gravity,
        PhysicsF32Field::JumpVelocityY,
        PhysicsF32Field::KnockbackResistance,
        PhysicsF32Field::BounceDampening,
        PhysicsF32Field::GroundFriction,
    ];
    let u32_fields = [
        PhysicsU32Field::KnockbackThreshold,
        PhysicsU32Field::BounceCount,
        PhysicsU32Field::HitRecoveryMs,
        PhysicsU32Field::LieDownDurationMs,
        PhysicsU32Field::RiseDurationMs,
        PhysicsU32Field::MaxJuggleCount,
        PhysicsU32Field::MaxDownHitCount,
        PhysicsU32Field::GuardBreakThreshold,
        PhysicsU32Field::GuardRecoveryMs,
    ];

    rsx! {
        div { class: "max-w-md w-full",
            div { class: "flex items-center justify-between mb-2",
                h2 { class: "text-xl font-semibold", "Physics" }
            }
            div { class: "collapse collapse-arrow bg-base-200",
                input { r#type: "checkbox" }
                div { class: "collapse-title text-sm text-base-content/70",
                    "重力 / ジャンプ / Knockback / バウンス / 摩擦 / timer / コンボ上限"
                }
                div { class: "collapse-content",
                    dl { class: "grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 items-center",
                        for f in f32_fields.iter().copied() {
                            dt {
                                class: "font-semibold text-base-content/70",
                                title: "{f.tooltip()}",
                                "{f.label()}"
                            }
                            dd {
                                EditPhysicsF32Inline { character: character.clone(), field: f }
                            }
                        }
                        for f in u32_fields.iter().copied() {
                            dt {
                                class: "font-semibold text-base-content/70",
                                title: "{f.tooltip()}",
                                "{f.label()}"
                            }
                            dd {
                                EditPhysicsU32Inline { character: character.clone(), field: f }
                            }
                        }
                    }
                }
            }
        }
    }
}
