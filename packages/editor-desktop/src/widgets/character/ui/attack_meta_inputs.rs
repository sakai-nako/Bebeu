//! AttackBox の meta (Damage / KnockbackDamage / Knockback Vec3 / HitStop) を編集する
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

use crate::shared::{AttackBoxMeta, HitStop, KnockbackVec};

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

    let input_class = "input input-bordered input-xs w-14";
    let label_class = "text-xs text-base-content/70";
    // grid 行のラベル列 (左) と値列 (右) の共通 class。値列は内部で flex-wrap 可。
    let row_label_class = "text-xs font-semibold text-base-content/80 self-center";
    let row_values_class = "flex flex-wrap items-center gap-2";

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

    // hit_stop 入力: 全 field が default なら meta.hit_stop = None、何か入力されれば Some に。
    // 既存 AttackBoxMeta の正規化と整合させる。
    let current_hs = current.hit_stop.unwrap_or_default();
    let apply_hit_stop = move |next_hs: HitStop| {
        let next_meta = AttackBoxMeta {
            hit_stop: if next_hs == HitStop::default() {
                None
            } else {
                Some(next_hs)
            },
            ..current
        };
        apply(next_meta);
    };

    // duration_ms は Option<u32> なので空文字で None / 数字で Some。
    let on_hs_duration = move |evt: Event<FormData>| {
        let raw = evt.value();
        let next_duration = if raw.trim().is_empty() {
            None
        } else if let Ok(v) = raw.trim().parse::<u32>() {
            Some(v)
        } else {
            return;
        };
        apply_hit_stop(HitStop {
            duration_ms: next_duration,
            ..current_hs
        });
    };
    let on_hs_shake_x = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<i32>() {
            apply_hit_stop(HitStop {
                shake_x: v,
                ..current_hs
            });
        }
    };
    let on_hs_shake_y = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<i32>() {
            apply_hit_stop(HitStop {
                shake_y: v,
                ..current_hs
            });
        }
    };
    let on_hs_count = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<u32>() {
            apply_hit_stop(HitStop {
                count: v,
                ..current_hs
            });
        }
    };
    let on_hs_decay = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<f32>() {
            // 0..=1 の範囲想定だが clamp は engine 側で行うので、ここでは入力をそのまま流す。
            apply_hit_stop(HitStop {
                decay: v,
                ..current_hs
            });
        }
    };

    // duration_ms 表示用 (None なら空欄)。
    let hs_duration_value = current_hs
        .duration_ms
        .map(|n| n.to_string())
        .unwrap_or_default();

    // ラベル + input の各ペアを内部 div で囲み、flex-wrap で折り返したときに
    // 「ラベルだけ前行末、input は次行頭」と分断されないようにする。各ペアは
    // 1 つの flex item として扱われるので一緒に折り返される。
    let pair_class = "flex items-center gap-1";
    // 各セクションは「見出し (上) → 値群 (下) の改行構成」。狭い panel 幅でも見出しと値が
    // 縦に重ならず読める。値群は内部で flex-wrap 可。
    rsx! {
        div { class: "space-y-2",
            // --- Damage section ---
            div { class: "space-y-1",
                h4 { class: "{row_label_class}", title: "ダメージ系", "Damage" }
                div { class: "{row_values_class}",
                    div { class: "{pair_class}",
                        span { class: "{label_class}", title: "HP 減算量", "DMG" }
                        input {
                            r#type: "number",
                            class: "{input_class}",
                            min: "0",
                            value: "{current.damage}",
                            onchange: on_damage,
                        }
                    }
                    div { class: "{pair_class}",
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
                    }
                }
            }

            // --- Knockback section ---
            div { class: "space-y-1",
                h4 {
                    class: "{row_label_class}",
                    title: "発動時の被弾側 VelX/Y/Z (px/s)",
                    "Knockback"
                }
                div { class: "{row_values_class}",
                    div { class: "{pair_class}",
                        span { class: "{label_class}", "x" }
                        input {
                            r#type: "number",
                            class: "{input_class}",
                            step: "any",
                            value: "{current.knockback.vel_x}",
                            onchange: on_vel_x,
                        }
                    }
                    div { class: "{pair_class}",
                        span { class: "{label_class}", "y" }
                        input {
                            r#type: "number",
                            class: "{input_class}",
                            step: "any",
                            value: "{current.knockback.vel_y}",
                            onchange: on_vel_y,
                        }
                    }
                    div { class: "{pair_class}",
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

            // --- HitStop section: ms / shake (x, y, count, decay) の 2 行 ---
            // 全 default なら meta.hit_stop = None として親に流す (= YAML 上 meta から省略)。
            // shake は片道 count 回ぶん三角波で揺らし、decay で振幅を線形に減衰させる。
            // 1 片道目はキャラ向き前方 (X) / 画面上 (Y) → 旧 impact の役割を内包する。
            div { class: "space-y-1",
                h4 {
                    class: "{row_label_class}",
                    title: "Hit 演出 (空欄で被弾側 Hit アニメ frame 0 duration を継承)",
                    "HitStop"
                }
                div { class: "{row_values_class}",
                    div { class: "{pair_class}",
                        span { class: "{label_class}", title: "duration (ms、空欄で継承)", "ms" }
                        input {
                            r#type: "number",
                            class: "{input_class}",
                            min: "0",
                            value: "{hs_duration_value}",
                            onchange: on_hs_duration,
                        }
                    }
                }
                div { class: "{row_values_class}",
                    div { class: "{pair_class}",
                        span { class: "{label_class}", title: "shake の初期振幅 X (前方+/後方-)", "shake x" }
                        input {
                            r#type: "number",
                            class: "{input_class}",
                            value: "{current_hs.shake_x}",
                            onchange: on_hs_shake_x,
                        }
                    }
                    div { class: "{pair_class}",
                        span { class: "{label_class}", title: "shake の初期振幅 Y (上+/下-)", "shake y" }
                        input {
                            r#type: "number",
                            class: "{input_class}",
                            value: "{current_hs.shake_y}",
                            onchange: on_hs_shake_y,
                        }
                    }
                    div { class: "{pair_class}",
                        span {
                            class: "{label_class}",
                            title: "片道回数 (中心 ↔ ±max を 1 と数える)。1=ガクッと 1 回押す、2=ガクッ→戻る、4=1 周期。0 で shake なし",
                            "count"
                        }
                        input {
                            r#type: "number",
                            class: "{input_class}",
                            min: "0",
                            value: "{current_hs.count}",
                            onchange: on_hs_count,
                        }
                    }
                    div { class: "{pair_class}",
                        span {
                            class: "{label_class}",
                            title: "振幅の線形減衰率 (0=一定、1=末尾で 0)",
                            "decay"
                        }
                        input {
                            r#type: "number",
                            class: "{input_class}",
                            min: "0",
                            max: "1",
                            step: "0.05",
                            value: "{current_hs.decay}",
                            onchange: on_hs_decay,
                        }
                    }
                }
            }
        }
    }
}
