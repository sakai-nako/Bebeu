//! AttackBox の meta (Damage / KnockbackDamage / HitstunExtra / Knockback Vec3) を編集する
//! 共通コンポーネント。
//!
//! Sprite の attack box editor (sprite_property_panel) と Frame override 用 BoxRow
//! (animation_property_panel/overrides) の両方から再利用される。値編集はすべて
//! `EventHandler<Option<AttackBoxMeta>>` 経由で親側に通知し、親側で永続化する。
//!
//! 「全フィールドが default = None として保存」「いずれかが非デフォルト = Some で保持」
//! の規約: 入力が全 0 / 0 ベクトルなら親に `None` を渡し、何か非ゼロの値が混じれば `Some`
//! を渡す。これで `AttackBox.has_meta()` の判定と一致し、YAML の serialize 時に
//! `meta` フィールドが省略される (= ダメージ無しと等価) ことを保証する。

use dioxus::prelude::*;

use crate::shared::{AttackBoxMeta, KnockbackVec};

/// AttackBox.meta を編集する 6 input 群。
///
/// - `meta`: 現在の値 (`None` なら「ダメージ無し」表示 = 全 0)。
/// - `on_change`: 編集後の値。すべて 0 なら `None`、何かが非ゼロなら `Some(...)`。
#[component]
pub(super) fn AttackMetaInputs(
    meta: Option<AttackBoxMeta>,
    on_change: EventHandler<Option<AttackBoxMeta>>,
) -> Element {
    let current = meta.unwrap_or_default();

    // 1 フィールド変更 → 新しい AttackBoxMeta を組み立て、全 0 なら None で親に通知する。
    let apply = move |next: AttackBoxMeta| {
        let normalized = if next == AttackBoxMeta::default() {
            None
        } else {
            Some(next)
        };
        if normalized == meta {
            return;
        }
        on_change.call(normalized);
    };

    let input_class = "input input-bordered input-xs w-16";
    let label_class = "text-xs text-base-content/70";

    let on_damage = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<u32>() {
            apply(AttackBoxMeta {
                damage: v,
                ..current
            });
        }
    };
    let on_kb_damage = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<u32>() {
            apply(AttackBoxMeta {
                knockback_damage: v,
                ..current
            });
        }
    };
    let on_hitstun = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<u32>() {
            apply(AttackBoxMeta {
                hitstun_extra_ms: v,
                ..current
            });
        }
    };
    let on_vel_x = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<f32>() {
            apply(AttackBoxMeta {
                knockback: KnockbackVec {
                    vel_x: v,
                    ..current.knockback
                },
                ..current
            });
        }
    };
    let on_vel_y = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<f32>() {
            apply(AttackBoxMeta {
                knockback: KnockbackVec {
                    vel_y: v,
                    ..current.knockback
                },
                ..current
            });
        }
    };
    let on_vel_z = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<f32>() {
            apply(AttackBoxMeta {
                knockback: KnockbackVec {
                    vel_z: v,
                    ..current.knockback
                },
                ..current
            });
        }
    };

    rsx! {
        div { class: "space-y-1",
            div { class: "flex flex-wrap items-center gap-1",
                span { class: "{label_class}", title: "HP 減算量", "DMG" }
                input {
                    r#type: "number",
                    class: "{input_class}",
                    min: "0",
                    value: "{current.damage}",
                    onchange: on_damage,
                }
                span {
                    class: "{label_class}",
                    title: "Knockback ゲージ減算量",
                    "KBD"
                }
                input {
                    r#type: "number",
                    class: "{input_class}",
                    min: "0",
                    value: "{current.knockback_damage}",
                    onchange: on_kb_damage,
                }
                span {
                    class: "{label_class}",
                    title: "Hit Animation 長への追加硬直 (ms)",
                    "+Hit"
                }
                input {
                    r#type: "number",
                    class: "{input_class}",
                    min: "0",
                    value: "{current.hitstun_extra_ms}",
                    onchange: on_hitstun,
                }
            }
            div { class: "flex flex-wrap items-center gap-1",
                span {
                    class: "{label_class}",
                    title: "発動時の被弾側 VelX/Y/Z (px/s)",
                    "Knockback"
                }
                span { class: "{label_class}", "x" }
                input {
                    r#type: "number",
                    class: "{input_class}",
                    step: "any",
                    value: "{current.knockback.vel_x}",
                    onchange: on_vel_x,
                }
                span { class: "{label_class}", "y" }
                input {
                    r#type: "number",
                    class: "{input_class}",
                    step: "any",
                    value: "{current.knockback.vel_y}",
                    onchange: on_vel_y,
                }
                span { class: "{label_class}", "z" }
                input {
                    r#type: "number",
                    class: "{input_class}",
                    step: "any",
                    value: "{current.knockback.vel_z}",
                    onchange: on_vel_z,
                }
            }
        }
    }
}
