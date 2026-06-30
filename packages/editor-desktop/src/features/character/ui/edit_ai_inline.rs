//! Character.ai (= `AiConfig`) の inline 編集 features (ADR-0035 Phase 5)。
//!
//! 構成:
//! - `EditAiKindInline`: None / Melee / Ally / Bot の kind dropdown。kind 切替時は新 variant の
//!   `default()` で初期化する (= ADR-0035 Phase 5 採用案、引き継ぎ仕様は持たない)。
//! - `EditAiSelectorInline`: `TargetSelector` dropdown (Nearest / LastEngaged / Random / WeightedByThreat)。
//! - `EditAiF32Inline` + `AiF32Field`: Engagement の f32 5 field + Ally の Follow 2 field を編集する
//!   inline (`EditPhysicsF32Inline` のパターン踏襲)。
//! - `EditAiU32Inline` + `AiU32Field`: Engagement の u32 3 field を編集する inline。

use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{
    AiConfig, AiKind, AllyConfig, BotConfig, Character, CharacterRepository, MeleeConfig,
    TargetSelector, use_characters_refresh,
};

// === kind dropdown =====================================================================

/// AI kind dropdown (None / Melee / Ally / Bot)。選択 → repo.update_metadata。
#[component]
pub fn EditAiKindInline(character: Character) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();
    let mut error = use_signal(|| None::<String>);

    let current_yaml = match &character.ai {
        None => "none",
        Some(c) => c.kind().yaml_kind(),
    };

    let on_change = {
        let original = character.clone();
        move |e: FormEvent| {
            let value = e.value();
            let mut updated = original.clone();
            updated.ai = if value == "none" {
                None
            } else if let Some(kind) = parse_kind(&value) {
                Some(kind.make_default())
            } else {
                error.set(Some(format!("未知の kind: {value}")));
                return;
            };
            match repo.update_metadata(&updated) {
                Ok(()) => {
                    refresh.bump();
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    rsx! {
        div { class: "flex items-center gap-2",
            select {
                class: "select select-bordered select-sm",
                value: "{current_yaml}",
                onchange: on_change,
                option { value: "none", "(none)" }
                for kind in [AiKind::Melee, AiKind::Ally, AiKind::Bot].iter().copied() {
                    option { value: "{kind.yaml_kind()}", "{kind.label()}" }
                }
            }
        }
        if let Some(message) = error() {
            p { class: "text-error text-xs mt-1", "{message}" }
        }
    }
}

fn parse_kind(s: &str) -> Option<AiKind> {
    match s {
        "melee" => Some(AiKind::Melee),
        "ally" => Some(AiKind::Ally),
        "bot" => Some(AiKind::Bot),
        _ => None,
    }
}

// === selector dropdown =================================================================

/// `TargetSelector` dropdown。kind 変更や ai=None のときは描画されない (= 呼び出し側で
/// `Character.ai` が `Some` のときだけ呼ぶ)。
#[component]
pub fn EditAiSelectorInline(character: Character) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();
    let mut error = use_signal(|| None::<String>);

    let current_yaml = match &character.ai {
        None => return rsx! {},
        Some(c) => selector_of(c).yaml_value(),
    };

    let on_change = {
        let original = character.clone();
        move |e: FormEvent| {
            let value = e.value();
            let Some(new_selector) = TargetSelector::from_yaml_value(&value) else {
                error.set(Some(format!("未知の selector: {value}")));
                return;
            };
            let mut updated = original.clone();
            let Some(ai) = updated.ai.as_mut() else {
                return;
            };
            set_selector(ai, new_selector);
            match repo.update_metadata(&updated) {
                Ok(()) => {
                    refresh.bump();
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    rsx! {
        div { class: "flex items-center gap-2",
            select {
                class: "select select-bordered select-sm",
                value: "{current_yaml}",
                onchange: on_change,
                for s in TargetSelector::all().iter().copied() {
                    option { value: "{s.yaml_value()}", "{s.label()}" }
                }
            }
        }
        if let Some(message) = error() {
            p { class: "text-error text-xs mt-1", "{message}" }
        }
    }
}

fn selector_of(ai: &AiConfig) -> TargetSelector {
    match ai {
        AiConfig::Melee(c) => c.selector,
        AiConfig::Ally(c) => c.selector,
        AiConfig::Bot(c) => c.selector,
    }
}

fn set_selector(ai: &mut AiConfig, s: TargetSelector) {
    match ai {
        AiConfig::Melee(c) => c.selector = s,
        AiConfig::Ally(c) => c.selector = s,
        AiConfig::Bot(c) => c.selector = s,
    }
}

// === f32 field enum + inline ===========================================================

/// AI Config の f32 field。Engagement 4 field (range 系) + Ally の Follow 2 field。
/// AllyConfig 固有 field は `is_applicable` で variant 制限する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiF32Field {
    ChaseEnterRangePx,
    ChaseExitRangePx,
    AttackEnterRangePx,
    AttackExitRangePx,
    /// AllyConfig 固有。Melee / Bot では非表示。
    FollowDistanceMinPx,
    /// AllyConfig 固有。
    FollowDistanceMaxPx,
}

impl AiF32Field {
    fn get(self, ai: &AiConfig) -> Option<f32> {
        match (self, ai) {
            (Self::ChaseEnterRangePx, AiConfig::Melee(c)) => {
                Some(c.engagement.chase_enter_range_px)
            }
            (Self::ChaseEnterRangePx, AiConfig::Ally(c)) => Some(c.engagement.chase_enter_range_px),
            (Self::ChaseEnterRangePx, AiConfig::Bot(c)) => Some(c.engagement.chase_enter_range_px),
            (Self::ChaseExitRangePx, AiConfig::Melee(c)) => Some(c.engagement.chase_exit_range_px),
            (Self::ChaseExitRangePx, AiConfig::Ally(c)) => Some(c.engagement.chase_exit_range_px),
            (Self::ChaseExitRangePx, AiConfig::Bot(c)) => Some(c.engagement.chase_exit_range_px),
            (Self::AttackEnterRangePx, AiConfig::Melee(c)) => {
                Some(c.engagement.attack_enter_range_px)
            }
            (Self::AttackEnterRangePx, AiConfig::Ally(c)) => {
                Some(c.engagement.attack_enter_range_px)
            }
            (Self::AttackEnterRangePx, AiConfig::Bot(c)) => {
                Some(c.engagement.attack_enter_range_px)
            }
            (Self::AttackExitRangePx, AiConfig::Melee(c)) => {
                Some(c.engagement.attack_exit_range_px)
            }
            (Self::AttackExitRangePx, AiConfig::Ally(c)) => Some(c.engagement.attack_exit_range_px),
            (Self::AttackExitRangePx, AiConfig::Bot(c)) => Some(c.engagement.attack_exit_range_px),
            (Self::FollowDistanceMinPx, AiConfig::Ally(c)) => Some(c.follow_distance_min_px),
            (Self::FollowDistanceMaxPx, AiConfig::Ally(c)) => Some(c.follow_distance_max_px),
            _ => None,
        }
    }

    fn set(self, ai: &mut AiConfig, v: f32) {
        match (self, ai) {
            (Self::ChaseEnterRangePx, AiConfig::Melee(c)) => c.engagement.chase_enter_range_px = v,
            (Self::ChaseEnterRangePx, AiConfig::Ally(c)) => c.engagement.chase_enter_range_px = v,
            (Self::ChaseEnterRangePx, AiConfig::Bot(c)) => c.engagement.chase_enter_range_px = v,
            (Self::ChaseExitRangePx, AiConfig::Melee(c)) => c.engagement.chase_exit_range_px = v,
            (Self::ChaseExitRangePx, AiConfig::Ally(c)) => c.engagement.chase_exit_range_px = v,
            (Self::ChaseExitRangePx, AiConfig::Bot(c)) => c.engagement.chase_exit_range_px = v,
            (Self::AttackEnterRangePx, AiConfig::Melee(c)) => {
                c.engagement.attack_enter_range_px = v
            }
            (Self::AttackEnterRangePx, AiConfig::Ally(c)) => c.engagement.attack_enter_range_px = v,
            (Self::AttackEnterRangePx, AiConfig::Bot(c)) => c.engagement.attack_enter_range_px = v,
            (Self::AttackExitRangePx, AiConfig::Melee(c)) => c.engagement.attack_exit_range_px = v,
            (Self::AttackExitRangePx, AiConfig::Ally(c)) => c.engagement.attack_exit_range_px = v,
            (Self::AttackExitRangePx, AiConfig::Bot(c)) => c.engagement.attack_exit_range_px = v,
            (Self::FollowDistanceMinPx, AiConfig::Ally(c)) => c.follow_distance_min_px = v,
            (Self::FollowDistanceMaxPx, AiConfig::Ally(c)) => c.follow_distance_max_px = v,
            _ => {}
        }
    }

    /// この field が現在の AiConfig variant に適用可能か。Melee / Bot で Follow 系は非適用。
    #[must_use]
    pub fn is_applicable(self, kind: AiKind) -> bool {
        match self {
            Self::FollowDistanceMinPx | Self::FollowDistanceMaxPx => kind == AiKind::Ally,
            _ => true,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ChaseEnterRangePx => "Chase Enter (px)",
            Self::ChaseExitRangePx => "Chase Exit (px)",
            Self::AttackEnterRangePx => "Attack Enter (px)",
            Self::AttackExitRangePx => "Attack Exit (px)",
            Self::FollowDistanceMinPx => "Follow Min (px)",
            Self::FollowDistanceMaxPx => "Follow Max (px)",
        }
    }

    #[must_use]
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::ChaseEnterRangePx => {
                "Chase に入る距離。chase_enter < chase_exit で hysteresis を作る"
            }
            Self::ChaseExitRangePx => "Chase から Idle に戻る距離。chase_enter より大きい値にする",
            Self::AttackEnterRangePx => "Attack 発火距離。attack_enter < attack_exit で hysteresis",
            Self::AttackExitRangePx => "Attack から Chase に戻る距離。attack_enter より大きい値",
            Self::FollowDistanceMinPx => "Player との距離がこれ未満で Follow を停止 (Ally 専用)",
            Self::FollowDistanceMaxPx => {
                "Player との距離がこれ超で Follow 再開。min < max (Ally 専用)"
            }
        }
    }
}

/// AI の f32 field を inline 編集するコンポーネント (`EditPhysicsF32Inline` と同形)。
#[component]
pub fn EditAiF32Inline(character: Character, field: AiF32Field) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let current = character
        .ai
        .as_ref()
        .and_then(|ai| field.get(ai))
        .unwrap_or(0.0);
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(move || format_f32(current));
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_edit = {
        let original = original.clone();
        move |_| {
            let v = original
                .ai
                .as_ref()
                .and_then(|ai| field.get(ai))
                .unwrap_or(0.0);
            draft.set(format_f32(v));
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
            let Some(ai) = updated.ai.as_mut() else {
                error.set(Some("AI が None のため編集できません".into()));
                return;
            };
            field.set(ai, new_value);
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
                    step: "1",
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

// === u32 field enum + inline ===========================================================

/// AI Config の u32 field。Engagement の 3 field (tick 系) を含む。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiU32Field {
    AttackCooldownTicks,
    DecisionIntervalTicks,
    MinDwellTicks,
}

impl AiU32Field {
    fn get(self, ai: &AiConfig) -> u32 {
        let eng = match ai {
            AiConfig::Melee(c) => &c.engagement,
            AiConfig::Ally(c) => &c.engagement,
            AiConfig::Bot(c) => &c.engagement,
        };
        match self {
            Self::AttackCooldownTicks => eng.attack_cooldown_ticks,
            Self::DecisionIntervalTicks => eng.decision_interval_ticks,
            Self::MinDwellTicks => eng.min_dwell_ticks,
        }
    }

    fn set(self, ai: &mut AiConfig, v: u32) {
        let eng = match ai {
            AiConfig::Melee(c) => &mut c.engagement,
            AiConfig::Ally(c) => &mut c.engagement,
            AiConfig::Bot(c) => &mut c.engagement,
        };
        match self {
            Self::AttackCooldownTicks => eng.attack_cooldown_ticks = v,
            Self::DecisionIntervalTicks => eng.decision_interval_ticks = v,
            Self::MinDwellTicks => eng.min_dwell_ticks = v,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::AttackCooldownTicks => "Attack Cooldown (ticks)",
            Self::DecisionIntervalTicks => "Decision Interval (ticks)",
            Self::MinDwellTicks => "Min Dwell (ticks)",
        }
    }

    #[must_use]
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::AttackCooldownTicks => {
                "攻撃発火後の cooldown (この間 Brain は attack:false を返す)。60 で 1 秒相当"
            }
            Self::DecisionIntervalTicks => {
                "N frame ごとに decision を回す。1 = 毎 frame、6 で約 10Hz"
            }
            Self::MinDwellTicks => "state 遷移後の最低滞在 tick (チャタリング防止)",
        }
    }
}

#[component]
pub fn EditAiU32Inline(character: Character, field: AiU32Field) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    let current = character.ai.as_ref().map_or(0, |ai| field.get(ai));
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(move || current.to_string());
    let mut error = use_signal(|| None::<String>);

    let original = character.clone();

    let on_edit = {
        let original = original.clone();
        move |_| {
            let v = original.ai.as_ref().map_or(0, |ai| field.get(ai));
            draft.set(v.to_string());
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
            let Some(ai) = updated.ai.as_mut() else {
                error.set(Some("AI が None のため編集できません".into()));
                return;
            };
            field.set(ai, new_value);
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

// === helper ============================================================================

fn format_f32(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_f32_field_round_trip_on_melee() {
        let mut ai = AiConfig::Melee(MeleeConfig::default());
        for f in [
            AiF32Field::ChaseEnterRangePx,
            AiF32Field::ChaseExitRangePx,
            AiF32Field::AttackEnterRangePx,
            AiF32Field::AttackExitRangePx,
        ] {
            f.set(&mut ai, 123.5);
            assert!((f.get(&ai).expect("applicable") - 123.5).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn ai_f32_field_follow_distance_is_ally_only() {
        let mut ai_melee = AiConfig::Melee(MeleeConfig::default());
        let mut ai_ally = AiConfig::Ally(AllyConfig::default());
        let mut ai_bot = AiConfig::Bot(BotConfig::default());

        // Melee / Bot では Follow field の get は None で、set は no-op。
        assert!(AiF32Field::FollowDistanceMinPx.get(&ai_melee).is_none());
        assert!(AiF32Field::FollowDistanceMinPx.get(&ai_bot).is_none());
        AiF32Field::FollowDistanceMinPx.set(&mut ai_melee, 999.0);
        AiF32Field::FollowDistanceMinPx.set(&mut ai_bot, 999.0);

        // Ally では正常に round-trip。
        AiF32Field::FollowDistanceMinPx.set(&mut ai_ally, 99.5);
        assert!(
            (AiF32Field::FollowDistanceMinPx
                .get(&ai_ally)
                .expect("applicable")
                - 99.5)
                .abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn ai_u32_field_round_trip_on_all_kinds() {
        for mut ai in [
            AiConfig::Melee(MeleeConfig::default()),
            AiConfig::Ally(AllyConfig::default()),
            AiConfig::Bot(BotConfig::default()),
        ] {
            for f in [
                AiU32Field::AttackCooldownTicks,
                AiU32Field::DecisionIntervalTicks,
                AiU32Field::MinDwellTicks,
            ] {
                f.set(&mut ai, 999);
                assert_eq!(f.get(&ai), 999);
            }
        }
    }

    #[test]
    fn is_applicable_filters_follow_fields_to_ally() {
        assert!(!AiF32Field::FollowDistanceMinPx.is_applicable(AiKind::Melee));
        assert!(!AiF32Field::FollowDistanceMinPx.is_applicable(AiKind::Bot));
        assert!(AiF32Field::FollowDistanceMinPx.is_applicable(AiKind::Ally));
        assert!(AiF32Field::ChaseEnterRangePx.is_applicable(AiKind::Melee));
        assert!(AiF32Field::ChaseEnterRangePx.is_applicable(AiKind::Bot));
    }

    #[test]
    fn format_f32_integer_no_decimal() {
        assert_eq!(format_f32(200.0), "200");
    }

    #[test]
    fn format_f32_fractional() {
        assert_eq!(format_f32(0.5), "0.5");
    }
}
