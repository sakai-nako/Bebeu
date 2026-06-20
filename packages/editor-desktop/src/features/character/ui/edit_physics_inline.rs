use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{
    Character, CharacterPhysics, CharacterRepository, use_characters_refresh,
};

/// f32 系の Physics フィールドを表す enum。表示ラベルと getter / setter を 1 箇所にまとめる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsF32Field {
    Gravity,
    JumpVelocityY,
    KnockbackResistance,
    BounceDampening,
    GroundFriction,
}

impl PhysicsF32Field {
    fn get(self, p: &CharacterPhysics) -> f32 {
        match self {
            Self::Gravity => p.gravity,
            Self::JumpVelocityY => p.jump_velocity_y,
            Self::KnockbackResistance => p.knockback_resistance,
            Self::BounceDampening => p.bounce_dampening,
            Self::GroundFriction => p.ground_friction,
        }
    }

    fn set(self, p: &mut CharacterPhysics, v: f32) {
        match self {
            Self::Gravity => p.gravity = v,
            Self::JumpVelocityY => p.jump_velocity_y = v,
            Self::KnockbackResistance => p.knockback_resistance = v,
            Self::BounceDampening => p.bounce_dampening = v,
            Self::GroundFriction => p.ground_friction = v,
        }
    }

    /// グリッド行の左ラベル (Properties の dt 相当)。
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Gravity => "Gravity",
            Self::JumpVelocityY => "Jump VelY",
            Self::KnockbackResistance => "Knockback Resist",
            Self::BounceDampening => "Bounce Dampen",
            Self::GroundFriction => "Ground Friction",
        }
    }

    /// ラベル / 入力欄の title 属性 (hover ヘルプ)。物理単位や使用条件を 1 文で説明。
    #[must_use]
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Gravity => {
                "重力加速度 (px/s²)。実効値は Level.gravity_scale を掛けたもの。既定 800.0"
            }
            Self::JumpVelocityY => "自発ジャンプ時の初速 (px/s)。既定 200.0",
            Self::KnockbackResistance => {
                "Knockback 軽減率 (0..1)。0=軽い・1=びくともしない。攻撃側 knockback ベクトルに (1 - resistance) が掛かる"
            }
            Self::BounceDampening => {
                "バウンス時の VelY 反転減衰率 (0..1)。0.5 で半分の高さまで跳ねる"
            }
            Self::GroundFriction => "地面 Slide 時の X/Z 摩擦 (px/s²)。既定 600.0",
        }
    }

    /// 数値入力の step 属性 (整数指定なし → 小数刻みでも入力可)。
    #[must_use]
    pub fn step(self) -> &'static str {
        match self {
            Self::Gravity | Self::JumpVelocityY | Self::GroundFriction => "1",
            Self::KnockbackResistance | Self::BounceDampening => "0.05",
        }
    }
}

/// u32 系の Physics フィールド。timer 系 (ms) とカウント系 (回数 / ポイント数) を含む。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsU32Field {
    KnockbackThreshold,
    MaxBounceCount,
    HitRecoveryMs,
    LieDownDurationMs,
    RiseDurationMs,
}

impl PhysicsU32Field {
    fn get(self, p: &CharacterPhysics) -> u32 {
        match self {
            Self::KnockbackThreshold => p.knockback_threshold,
            Self::MaxBounceCount => p.max_bounce_count,
            Self::HitRecoveryMs => p.hit_recovery_ms,
            Self::LieDownDurationMs => p.lie_down_duration_ms,
            Self::RiseDurationMs => p.rise_duration_ms,
        }
    }

    fn set(self, p: &mut CharacterPhysics, v: u32) {
        match self {
            Self::KnockbackThreshold => p.knockback_threshold = v,
            Self::MaxBounceCount => p.max_bounce_count = v,
            Self::HitRecoveryMs => p.hit_recovery_ms = v,
            Self::LieDownDurationMs => p.lie_down_duration_ms = v,
            Self::RiseDurationMs => p.rise_duration_ms = v,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::KnockbackThreshold => "Knockback Gauge",
            Self::MaxBounceCount => "Max Bounces",
            Self::HitRecoveryMs => "Hit Recovery (ms)",
            Self::LieDownDurationMs => "LieDown (ms)",
            Self::RiseDurationMs => "Rise (ms)",
        }
    }

    #[must_use]
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::KnockbackThreshold => {
                "Knockback ゲージの最大値。Hit を受けるたび knockback_damage で減算され、0 以下で吹っ飛び発動 → full 回復"
            }
            Self::MaxBounceCount => {
                "バウンス回数の上限。0 でバウンス無効 (Slide に直行)。1 で 1 回バウンス"
            }
            Self::HitRecoveryMs => {
                "Hit (地上小硬直) 後、Knockback ゲージが full 回復するまでの待ち (ms)"
            }
            Self::LieDownDurationMs => {
                "対応 Role: LieDown (Animation が is_loop=true または未登録時のみ使用)。is_loop=false の単発 Animation を登録した場合は Animation 長が優先される"
            }
            Self::RiseDurationMs => {
                "対応 Role: Rise (Animation が is_loop=true または未登録時のみ使用)。is_loop=false の単発 Animation を登録した場合は Animation 長が優先される"
            }
        }
    }
}

/// `f32` 系 Physics フィールドを inline 編集する共通コンポーネント。
/// EditHpInline と同じ「表示 ↔ 編集モード切替」パターンで、Repository.update_metadata に書く。
#[component]
pub fn EditPhysicsF32Inline(character: Character, field: PhysicsF32Field) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let current = field.get(&character.physics);
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(move || format_f32(current));
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_edit = {
        let original = original.clone();
        move |_| {
            draft.set(format_f32(field.get(&original.physics)));
            error.set(None);
            editing.set(true);
        }
    };

    let on_save = {
        let original = original.clone();
        move |_| {
            let Ok(new_value) = draft().trim().parse::<f32>() else {
                error.set(Some("有効な数値を入力してください".into()));
                return;
            };
            if !new_value.is_finite() {
                error.set(Some("有効な数値を入力してください (NaN / Inf 不可)".into()));
                return;
            }
            let mut updated = original.clone();
            field.set(&mut updated.physics, new_value);
            match repo.update_metadata(&updated) {
                Ok(()) => {
                    refresh.bump();
                    editing.set(false);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    let on_cancel = move |_| {
        editing.set(false);
        error.set(None);
    };

    let step = field.step();

    rsx! {
        if editing() {
            div { class: "flex items-center gap-2",
                input {
                    r#type: "number",
                    class: "input input-bordered input-sm w-28",
                    value: "{draft}",
                    step: "{step}",
                    oninput: move |e| draft.set(e.value()),
                }
                button {
                    r#type: "button",
                    class: "btn btn-primary btn-xs",
                    onclick: on_save,
                    "Save"
                }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: on_cancel,
                    "Cancel"
                }
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        } else {
            div { class: "flex items-center gap-2",
                span { "{format_f32(current)}" }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: on_edit,
                    title: "編集",
                    "✎"
                }
            }
        }
    }
}

/// `u32` 系 Physics フィールドを inline 編集する共通コンポーネント。
#[component]
pub fn EditPhysicsU32Inline(character: Character, field: PhysicsU32Field) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let current = field.get(&character.physics);
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(move || current.to_string());
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_edit = {
        let original = original.clone();
        move |_| {
            draft.set(field.get(&original.physics).to_string());
            error.set(None);
            editing.set(true);
        }
    };

    let on_save = {
        let original = original.clone();
        move |_| {
            let Ok(new_value) = draft().trim().parse::<u32>() else {
                error.set(Some("0 以上の整数で入力してください".into()));
                return;
            };
            let mut updated = original.clone();
            field.set(&mut updated.physics, new_value);
            match repo.update_metadata(&updated) {
                Ok(()) => {
                    refresh.bump();
                    editing.set(false);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    let on_cancel = move |_| {
        editing.set(false);
        error.set(None);
    };

    rsx! {
        if editing() {
            div { class: "flex items-center gap-2",
                input {
                    r#type: "number",
                    class: "input input-bordered input-sm w-28",
                    value: "{draft}",
                    min: "0",
                    oninput: move |e| draft.set(e.value()),
                }
                button {
                    r#type: "button",
                    class: "btn btn-primary btn-xs",
                    onclick: on_save,
                    "Save"
                }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: on_cancel,
                    "Cancel"
                }
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        } else {
            div { class: "flex items-center gap-2",
                span { "{current}" }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: on_edit,
                    title: "編集",
                    "✎"
                }
            }
        }
    }
}

/// f32 値を表示用に整形する。整数値は `.0` を省く (e.g. `800` vs `800.5`)、それ以外は精度 3 桁。
fn format_f32(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        // 末尾 0 を抑制しつつ 3 桁精度
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_f32_integer_no_decimal() {
        assert_eq!(format_f32(800.0), "800");
        assert_eq!(format_f32(0.0), "0");
    }

    #[test]
    fn format_f32_fractional_trims_trailing_zeros() {
        assert_eq!(format_f32(0.5), "0.5");
        assert_eq!(format_f32(0.125), "0.125");
    }

    #[test]
    fn physics_f32_field_round_trip() {
        let mut p = CharacterPhysics::default();
        for f in [
            PhysicsF32Field::Gravity,
            PhysicsF32Field::JumpVelocityY,
            PhysicsF32Field::KnockbackResistance,
            PhysicsF32Field::BounceDampening,
            PhysicsF32Field::GroundFriction,
        ] {
            f.set(&mut p, 123.5);
            assert!((f.get(&p) - 123.5).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn physics_u32_field_round_trip() {
        let mut p = CharacterPhysics::default();
        for f in [
            PhysicsU32Field::KnockbackThreshold,
            PhysicsU32Field::MaxBounceCount,
            PhysicsU32Field::HitRecoveryMs,
            PhysicsU32Field::LieDownDurationMs,
            PhysicsU32Field::RiseDurationMs,
        ] {
            f.set(&mut p, 999);
            assert_eq!(f.get(&p), 999);
        }
    }
}
