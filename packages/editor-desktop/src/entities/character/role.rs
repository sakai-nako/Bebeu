//! Animation の役割 (Role) と役割内 slot (variant) を扱う semantic タグ。
//!
//! 「Animation.number」「Animation.name」はエンジン毎に異なる慣習で解釈されてきたが、
//! role を single source of truth として導入することで、各エンジン側は role + variant →
//! 自分の番号体系/名前体系 への写像テーブルを持つだけで済むようになった。
//!
//! - 1 Animation = 1 role (兼用は許さない)。
//! - Custom は unit variant の「役割なし」スロット。AI scripting / カスタム必殺技用。
//!   Custom の Animation を ikemen export で参照したい場合は `Animation.export_number` を
//!   手動で持たせる (model.rs 参照)。
//! - Single-cardinality role (Idle/Walk/Guard) は variant=0 のみ。Multi-cardinality role
//!   (Attack/Hit/Dead/Jump) は variant 0,1,2,... で弱/中/強や down 種別を区別する。
//! - Walk の前後区別が必要になったら BackWalk role を別途追加する想定 (今回は enum に入れない)。

use serde::{Deserialize, Serialize};

use super::Animation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Idle,
    Walk,
    Attack,
    /// 立ちガード (ADR-0028)。旧 `block` YAML は serde alias でこの variant に読み替える。
    #[serde(alias = "block")]
    Guard,
    /// ガードクラッシュ (ADR-0028)。`guard_gauge <= 0` で発動する 1 frame 中継 Role。
    /// engine 側で次フレームに `KnockbackUp` に書き換わるため Animation 終端は意識しない。
    GuardBreak,
    Jump,
    /// 空中攻撃 (ADR-0027)。`Jump` 中の `J` / `Space` で発動する Attack 系の独立 Role。
    JumpAttack,
    Hit,
    // Knockback フロー (通常被弾時の物理ステージ群)。すべて single-cardinality。
    KnockbackUp,
    KnockbackDown,
    BounceUp,
    BounceDown,
    Slide,
    LieDown,
    Rise,
    // Back 系フロー (背後被弾。前のめりに吹っ飛ぶ)。すべて single-cardinality。
    // engine 側 ResolveAnimation が被弾方向で出し分け、未登録なら正面版 (KnockbackUp 等) に fallback。
    BackKnockbackUp,
    BackKnockbackDown,
    BackBounceUp,
    BackBounceDown,
    BackSlide,
    BackLieDown,
    BackRise,
    // Dead 系フロー (HP=0 被弾時の死亡演出)。Animation 解決層で通常 Role に fallback する。
    // すべて multi-cardinality (死因 variant を将来許容する設計)。
    // 旧 `Role::Dead` は `DeadLieDown` に役割を移管 (serde alias で旧 YAML を読み替え)。
    DeadKnockbackUp,
    DeadKnockbackDown,
    DeadBounceUp,
    DeadBounceDown,
    DeadSlide,
    /// 死亡時の最終静止。Rise には進まず Animation 末尾で永続停止する。
    /// 旧形式 (`role: dead`) は alias でこの variant に読み替えられる。
    #[serde(alias = "dead")]
    DeadLieDown,
    // DeadBack 系フロー (致命傷 × 背後被弾)。Dead 系同様 Rise なし、すべて multi-cardinality。
    // 未登録なら Dead 系 → Back 系 → 正面版の順に fallback する (engine 側 ResolveAnimation)。
    DeadBackKnockbackUp,
    DeadBackKnockbackDown,
    DeadBackBounceUp,
    DeadBackBounceDown,
    DeadBackSlide,
    DeadBackLieDown,
    /// Down 中 (Slide / LieDown / Rise) に被弾したときの地上 hit ポーズ。Hit が立ちポーズ
    /// 前提なので、地面に伏せている被弾には別 role を割り当てる。single-cardinality。
    DownHit,
    /// 下段攻撃ポーズ (`K` キー)。AttackBox を低位置に置いて倒れた敵 (LieDown) に当てる用。
    /// single-cardinality。
    DownAttack,
    /// 役割なしスロット。エンジン側の State には流れず、AI scripting 等で number/name 経由で参照される。
    #[default]
    Custom,
}

/// Animation Role がどう終了判定されるかの種別。editor UI が「終了条件ステータス行」を
/// リアルタイム表示するためのキー (Phase 6 で実体表示)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminatorKind {
    /// 物理で終了 (VelY 符号 / X/Z 摩擦)。Animation 自体は is_loop=true 推奨。
    PhysicsDriven,
    /// is_loop=false なら Animation 終端で次へ、true / 未登録なら fallback timer で次へ。
    AnimationLengthOrTimer,
    /// Animation 終端で永続停止 (Rise しない)。Dead 用。
    AnimationLengthPersistent,
    /// 既存挙動 (Animation 終端で Idle、または入力駆動)。Role に応じた専用の終了規約はなし。
    Generic,
}

impl TerminatorKind {
    /// Role セレクタの option label に併記するブラケット内の短い説明。`Generic` は併記しない
    /// (= 既存挙動なので情報量が低い)。Phase 6 で追加する「終了条件ステータス行」と語感を
    /// 揃えるための単一情報源。
    #[must_use]
    pub const fn short_hint(self) -> Option<&'static str> {
        match self {
            Self::PhysicsDriven => Some("物理駆動"),
            Self::AnimationLengthOrTimer => Some("Animation長 or timer"),
            Self::AnimationLengthPersistent => Some("Animation長で永続停止"),
            Self::Generic => None,
        }
    }
}

impl Role {
    /// 全 role を UI セレクタ等で列挙するための順序付き配列。
    /// 並び順は「基本」→「Knockback フロー (通常)」→「Dead フロー」→「Custom」。
    pub const fn all() -> &'static [Role] {
        &[
            Role::Idle,
            Role::Walk,
            Role::Attack,
            Role::Guard,
            Role::GuardBreak,
            Role::Jump,
            Role::JumpAttack,
            Role::Hit,
            Role::KnockbackUp,
            Role::KnockbackDown,
            Role::BounceUp,
            Role::BounceDown,
            Role::Slide,
            Role::LieDown,
            Role::Rise,
            Role::BackKnockbackUp,
            Role::BackKnockbackDown,
            Role::BackBounceUp,
            Role::BackBounceDown,
            Role::BackSlide,
            Role::BackLieDown,
            Role::BackRise,
            Role::DeadKnockbackUp,
            Role::DeadKnockbackDown,
            Role::DeadBounceUp,
            Role::DeadBounceDown,
            Role::DeadSlide,
            Role::DeadLieDown,
            Role::DeadBackKnockbackUp,
            Role::DeadBackKnockbackDown,
            Role::DeadBackBounceUp,
            Role::DeadBackBounceDown,
            Role::DeadBackSlide,
            Role::DeadBackLieDown,
            Role::DownHit,
            Role::DownAttack,
            Role::Custom,
        ]
    }

    /// daisyUI badge 等に出す表示ラベル。
    #[must_use]
    pub const fn display_label(self) -> &'static str {
        match self {
            Role::Idle => "Idle",
            Role::Walk => "Walk",
            Role::Attack => "Attack",
            Role::Guard => "Guard",
            Role::GuardBreak => "GuardBreak",
            Role::Jump => "Jump",
            Role::JumpAttack => "JumpAttack",
            Role::Hit => "Hit",
            Role::KnockbackUp => "KnockbackUp",
            Role::KnockbackDown => "KnockbackDown",
            Role::BounceUp => "BounceUp",
            Role::BounceDown => "BounceDown",
            Role::Slide => "Slide",
            Role::LieDown => "LieDown",
            Role::Rise => "Rise",
            Role::BackKnockbackUp => "BackKnockbackUp",
            Role::BackKnockbackDown => "BackKnockbackDown",
            Role::BackBounceUp => "BackBounceUp",
            Role::BackBounceDown => "BackBounceDown",
            Role::BackSlide => "BackSlide",
            Role::BackLieDown => "BackLieDown",
            Role::BackRise => "BackRise",
            Role::DeadKnockbackUp => "DeadKnockbackUp",
            Role::DeadKnockbackDown => "DeadKnockbackDown",
            Role::DeadBounceUp => "DeadBounceUp",
            Role::DeadBounceDown => "DeadBounceDown",
            Role::DeadSlide => "DeadSlide",
            Role::DeadLieDown => "DeadLieDown",
            Role::DeadBackKnockbackUp => "DeadBackKnockbackUp",
            Role::DeadBackKnockbackDown => "DeadBackKnockbackDown",
            Role::DeadBackBounceUp => "DeadBackBounceUp",
            Role::DeadBackBounceDown => "DeadBackBounceDown",
            Role::DeadBackSlide => "DeadBackSlide",
            Role::DeadBackLieDown => "DeadBackLieDown",
            Role::DownHit => "DownHit",
            Role::DownAttack => "DownAttack",
            Role::Custom => "Custom",
        }
    }

    /// Single-cardinality (1 character につき 1 個まで) の role か。
    /// Single の場合は variant フィールドは使われず、UI 上 disabled にする。
    ///
    /// Knockback フロー 7 個 (KnockbackUp / Down / BounceUp / Down / Slide / LieDown / Rise) は
    /// すべて single。Dead 系 6 個は multi (将来 variant で死因を分岐する余地を残す)。
    #[must_use]
    pub const fn is_single_cardinality(self) -> bool {
        matches!(
            self,
            Role::Idle
                | Role::Walk
                | Role::Guard
                | Role::GuardBreak
                | Role::Jump
                | Role::JumpAttack
                | Role::KnockbackUp
                | Role::KnockbackDown
                | Role::BounceUp
                | Role::BounceDown
                | Role::Slide
                | Role::LieDown
                | Role::Rise
                | Role::BackKnockbackUp
                | Role::BackKnockbackDown
                | Role::BackBounceUp
                | Role::BackBounceDown
                | Role::BackSlide
                | Role::BackLieDown
                | Role::BackRise
                | Role::DownHit
                | Role::DownAttack
                | Role::Custom
        )
    }

    /// snake_case の YAML 表現を返す (`role: knockback_up` 等)。serde の serialize と同じ値だが、
    /// `<select value>` のような UI 用途で文字列比較する場合に serde 経由を回避できる軽量 API。
    #[must_use]
    pub const fn yaml_value(self) -> &'static str {
        match self {
            Role::Idle => "idle",
            Role::Walk => "walk",
            Role::Attack => "attack",
            Role::Guard => "guard",
            Role::GuardBreak => "guard_break",
            Role::Jump => "jump",
            Role::JumpAttack => "jump_attack",
            Role::Hit => "hit",
            Role::KnockbackUp => "knockback_up",
            Role::KnockbackDown => "knockback_down",
            Role::BounceUp => "bounce_up",
            Role::BounceDown => "bounce_down",
            Role::Slide => "slide",
            Role::LieDown => "lie_down",
            Role::Rise => "rise",
            Role::BackKnockbackUp => "back_knockback_up",
            Role::BackKnockbackDown => "back_knockback_down",
            Role::BackBounceUp => "back_bounce_up",
            Role::BackBounceDown => "back_bounce_down",
            Role::BackSlide => "back_slide",
            Role::BackLieDown => "back_lie_down",
            Role::BackRise => "back_rise",
            Role::DeadKnockbackUp => "dead_knockback_up",
            Role::DeadKnockbackDown => "dead_knockback_down",
            Role::DeadBounceUp => "dead_bounce_up",
            Role::DeadBounceDown => "dead_bounce_down",
            Role::DeadSlide => "dead_slide",
            Role::DeadLieDown => "dead_lie_down",
            Role::DeadBackKnockbackUp => "dead_back_knockback_up",
            Role::DeadBackKnockbackDown => "dead_back_knockback_down",
            Role::DeadBackBounceUp => "dead_back_bounce_up",
            Role::DeadBackBounceDown => "dead_back_bounce_down",
            Role::DeadBackSlide => "dead_back_slide",
            Role::DeadBackLieDown => "dead_back_lie_down",
            Role::DownHit => "down_hit",
            Role::DownAttack => "down_attack",
            Role::Custom => "custom",
        }
    }

    /// YAML 表現 (snake_case) から Role を引く。旧 `"dead"` も `DeadLieDown` として読み替える (serde alias と同じ規約)。
    #[must_use]
    pub fn from_yaml_value(s: &str) -> Option<Role> {
        Some(match s {
            "idle" => Role::Idle,
            "walk" => Role::Walk,
            "attack" => Role::Attack,
            // ADR-0028: `block` は旧 YAML の alias として残し、`guard` を正規表現とする。
            "guard" | "block" => Role::Guard,
            "guard_break" => Role::GuardBreak,
            "jump" => Role::Jump,
            "jump_attack" => Role::JumpAttack,
            "hit" => Role::Hit,
            "knockback_up" => Role::KnockbackUp,
            "knockback_down" => Role::KnockbackDown,
            "bounce_up" => Role::BounceUp,
            "bounce_down" => Role::BounceDown,
            "slide" => Role::Slide,
            "lie_down" => Role::LieDown,
            "rise" => Role::Rise,
            "back_knockback_up" => Role::BackKnockbackUp,
            "back_knockback_down" => Role::BackKnockbackDown,
            "back_bounce_up" => Role::BackBounceUp,
            "back_bounce_down" => Role::BackBounceDown,
            "back_slide" => Role::BackSlide,
            "back_lie_down" => Role::BackLieDown,
            "back_rise" => Role::BackRise,
            "dead_knockback_up" => Role::DeadKnockbackUp,
            "dead_knockback_down" => Role::DeadKnockbackDown,
            "dead_bounce_up" => Role::DeadBounceUp,
            "dead_bounce_down" => Role::DeadBounceDown,
            "dead_slide" => Role::DeadSlide,
            "dead_lie_down" | "dead" => Role::DeadLieDown,
            "dead_back_knockback_up" => Role::DeadBackKnockbackUp,
            "dead_back_knockback_down" => Role::DeadBackKnockbackDown,
            "dead_back_bounce_up" => Role::DeadBackBounceUp,
            "dead_back_bounce_down" => Role::DeadBackBounceDown,
            "dead_back_slide" => Role::DeadBackSlide,
            "dead_back_lie_down" => Role::DeadBackLieDown,
            "down_hit" => Role::DownHit,
            "down_attack" => Role::DownAttack,
            "custom" => Role::Custom,
            _ => return None,
        })
    }

    /// Role セレクタの option label に出す表示文字列。`terminator_kind()` に応じて
    /// `"KnockbackUp [物理駆動]"` のようなブラケット表記を併記する。`Generic` の Role
    /// (Idle / Walk / Attack / Hit / Guard / GuardBreak / Jump / JumpAttack / Custom) は
    /// ブラケット併記なし。
    #[must_use]
    pub fn selector_label(self) -> String {
        match self.terminator_kind().short_hint() {
            Some(hint) => format!("{} [{hint}]", self.display_label()),
            None => self.display_label().to_string(),
        }
    }

    /// この Role の Animation がどう終了判定されるかを返す。
    /// editor UI の「終了条件ステータス行」と Physics セクション tooltip の整合性を取るための単一情報源。
    #[must_use]
    pub const fn terminator_kind(self) -> TerminatorKind {
        match self {
            Role::KnockbackUp
            | Role::KnockbackDown
            | Role::BounceUp
            | Role::BounceDown
            | Role::Slide
            | Role::BackKnockbackUp
            | Role::BackKnockbackDown
            | Role::BackBounceUp
            | Role::BackBounceDown
            | Role::BackSlide
            | Role::DeadKnockbackUp
            | Role::DeadKnockbackDown
            | Role::DeadBounceUp
            | Role::DeadBounceDown
            | Role::DeadSlide
            | Role::DeadBackKnockbackUp
            | Role::DeadBackKnockbackDown
            | Role::DeadBackBounceUp
            | Role::DeadBackBounceDown
            | Role::DeadBackSlide => TerminatorKind::PhysicsDriven,
            Role::LieDown | Role::Rise | Role::BackLieDown | Role::BackRise | Role::DownHit => {
                TerminatorKind::AnimationLengthOrTimer
            }
            Role::DeadLieDown | Role::DeadBackLieDown => TerminatorKind::AnimationLengthPersistent,
            Role::Idle
            | Role::Walk
            | Role::Attack
            | Role::Hit
            | Role::Guard
            | Role::GuardBreak
            | Role::Jump
            | Role::JumpAttack
            | Role::DownAttack
            | Role::Custom => TerminatorKind::Generic,
        }
    }
}

/// `validate_animations` が返す違反種別。Severity は `severity()` で取れる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleViolation {
    /// Single-cardinality role に複数の Animation が割り当たっている (error)。
    DuplicateSingleRole {
        role: Role,
        animation_names: Vec<String>,
    },
    /// Multi-cardinality role の同じ variant に複数の Animation が割り当たっている (error)。
    DuplicateRoleVariant {
        role: Role,
        variant: u32,
        animation_names: Vec<String>,
    },
    /// Multi-cardinality role の variant に飛びがある (warn、ikemen export 時に致命傷にはならない)。
    VariantGap { role: Role, missing: u32 },
    /// Custom role の `export_number` が複数 Animation で重複している (error)。
    /// ikemen export 時に同じ Action 番号で複数ブロックが衝突するため。
    DuplicateExportNumber {
        export_number: u32,
        animation_names: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}

impl RoleViolation {
    #[must_use]
    pub fn severity(&self) -> Severity {
        match self {
            RoleViolation::DuplicateSingleRole { .. }
            | RoleViolation::DuplicateRoleVariant { .. }
            | RoleViolation::DuplicateExportNumber { .. } => Severity::Error,
            RoleViolation::VariantGap { .. } => Severity::Warn,
        }
    }
}

/// Animation 配列の role 割当を検証する。Custom role 自体は role 衝突の対象外だが、
/// export_number が複数 Custom Animation で重複している場合は error にする。
///
/// - Single-cardinality role に 2 件以上 → `DuplicateSingleRole` (error)
/// - Multi-cardinality role の同 variant 重複 → `DuplicateRoleVariant` (error)
/// - Multi-cardinality role の variant 飛び (e.g. 0, 2 で 1 抜け) → `VariantGap` (warn)
/// - Custom role の `export_number` 重複 → `DuplicateExportNumber` (error)
#[must_use]
pub fn validate_animations(animations: &[Animation]) -> Vec<RoleViolation> {
    use std::collections::BTreeMap;

    let mut by_role: BTreeMap<u8, (Role, Vec<&Animation>)> = BTreeMap::new();
    for anim in animations {
        if anim.role == Role::Custom {
            continue;
        }
        by_role
            .entry(anim.role as u8)
            .or_insert_with(|| (anim.role, Vec::new()))
            .1
            .push(anim);
    }

    let mut violations = Vec::new();
    for (_, (role, anims)) in by_role {
        if role.is_single_cardinality() {
            if anims.len() > 1 {
                violations.push(RoleViolation::DuplicateSingleRole {
                    role,
                    animation_names: anims.iter().map(|a| a.name.clone()).collect(),
                });
            }
            continue;
        }

        // multi-cardinality: variant 重複と飛びをチェック
        let mut by_variant: BTreeMap<u32, Vec<&Animation>> = BTreeMap::new();
        for a in &anims {
            by_variant.entry(a.variant).or_default().push(a);
        }
        for (variant, vanims) in &by_variant {
            if vanims.len() > 1 {
                violations.push(RoleViolation::DuplicateRoleVariant {
                    role,
                    variant: *variant,
                    animation_names: vanims.iter().map(|a| a.name.clone()).collect(),
                });
            }
        }
        if let Some(&max) = by_variant.keys().max() {
            for v in 0..max {
                if !by_variant.contains_key(&v) {
                    violations.push(RoleViolation::VariantGap { role, missing: v });
                }
            }
        }
    }

    // Custom role の export_number 重複チェック (None は無視)。
    let mut by_export: BTreeMap<u32, Vec<&Animation>> = BTreeMap::new();
    for anim in animations {
        if anim.role != Role::Custom {
            continue;
        }
        if let Some(n) = anim.export_number {
            by_export.entry(n).or_default().push(anim);
        }
    }
    for (export_number, anims) in by_export {
        if anims.len() > 1 {
            violations.push(RoleViolation::DuplicateExportNumber {
                export_number,
                animation_names: anims.iter().map(|a| a.name.clone()).collect(),
            });
        }
    }

    violations
}

/// Character 全体の保存前バリデーション。現状は `validate_animations` への薄いラッパだが、
/// 将来 SpriteGroup / SoundGroup の role 的概念が増えたときの拡張点として用意しておく。
#[must_use]
pub fn validate_for_save(animations: &[Animation]) -> Vec<RoleViolation> {
    validate_animations(animations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::character::Animation;

    fn anim(name: &str, role: Role, variant: u32) -> Animation {
        Animation {
            name: name.to_string(),
            role,
            variant,
            export_number: None,
            is_loop: true,
            loop_start_index: 0,
            frames: Vec::new(),
        }
    }

    fn custom_anim(name: &str, export_number: Option<u32>) -> Animation {
        Animation {
            name: name.to_string(),
            role: Role::Custom,
            variant: 0,
            export_number,
            is_loop: true,
            loop_start_index: 0,
            frames: Vec::new(),
        }
    }

    #[test]
    fn validates_empty_animations() {
        assert!(validate_animations(&[]).is_empty());
    }

    #[test]
    fn skips_custom_role() {
        let anims = vec![
            anim("ai_special", Role::Custom, 0),
            anim("ai_taunt", Role::Custom, 0),
        ];
        assert!(validate_animations(&anims).is_empty());
    }

    #[test]
    fn rejects_duplicate_single_role() {
        let anims = vec![anim("idle1", Role::Idle, 0), anim("idle2", Role::Idle, 0)];
        let v = validate_animations(&anims);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity(), Severity::Error);
        assert!(matches!(v[0], RoleViolation::DuplicateSingleRole { .. }));
    }

    #[test]
    fn rejects_duplicate_role_variant() {
        let anims = vec![
            anim("attack_a", Role::Attack, 0),
            anim("attack_b", Role::Attack, 0),
            anim("attack_c", Role::Attack, 1),
        ];
        let v = validate_animations(&anims);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity(), Severity::Error);
        assert!(matches!(
            v[0],
            RoleViolation::DuplicateRoleVariant { variant: 0, .. }
        ));
    }

    #[test]
    fn warns_on_variant_gap() {
        let anims = vec![
            anim("attack_a", Role::Attack, 0),
            anim("attack_c", Role::Attack, 2),
        ];
        let v = validate_animations(&anims);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity(), Severity::Warn);
        assert!(matches!(
            v[0],
            RoleViolation::VariantGap {
                role: Role::Attack,
                missing: 1
            }
        ));
    }

    #[test]
    fn rejects_duplicate_export_number_for_custom() {
        let anims = vec![
            custom_anim("a", Some(1000)),
            custom_anim("b", Some(1000)),
            custom_anim("c", Some(1001)),
        ];
        let v = validate_animations(&anims);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity(), Severity::Error);
        assert!(matches!(
            v[0],
            RoleViolation::DuplicateExportNumber {
                export_number: 1000,
                ..
            }
        ));
    }

    #[test]
    fn accepts_custom_without_export_number() {
        // export_number が None の Custom は何個あっても重複扱いしない。
        let anims = vec![custom_anim("a", None), custom_anim("b", None)];
        assert!(validate_animations(&anims).is_empty());
    }

    #[test]
    fn accepts_well_formed_attacks() {
        let anims = vec![
            anim("light", Role::Attack, 0),
            anim("medium", Role::Attack, 1),
            anim("heavy", Role::Attack, 2),
        ];
        assert!(validate_animations(&anims).is_empty());
    }

    // Phase 8: 全 Role が yaml_value ↔ from_yaml_value で往復できることを保証する
    // (新 Back 系 / DeadBack 系を含む。どちらかに書き漏れがあれば落ちる回帰ガード)。
    #[test]
    fn yaml_value_round_trips_for_all_roles() {
        for r in Role::all().iter().copied() {
            let s = r.yaml_value();
            assert_eq!(
                Role::from_yaml_value(s),
                Some(r),
                "round-trip failed for {s}"
            );
        }
    }

    // Phase 8: Back 系 7 個は single-cardinality (正面版と対称)、DeadBack 系 6 個は
    // multi-cardinality (Dead 系と対称) であることを確認する。
    #[test]
    fn back_roles_single_dead_back_roles_multi() {
        for r in [
            Role::BackKnockbackUp,
            Role::BackKnockbackDown,
            Role::BackBounceUp,
            Role::BackBounceDown,
            Role::BackSlide,
            Role::BackLieDown,
            Role::BackRise,
        ] {
            assert!(
                r.is_single_cardinality(),
                "{} should be single-cardinality",
                r.display_label()
            );
        }
        for r in [
            Role::DeadBackKnockbackUp,
            Role::DeadBackKnockbackDown,
            Role::DeadBackBounceUp,
            Role::DeadBackBounceDown,
            Role::DeadBackSlide,
            Role::DeadBackLieDown,
        ] {
            assert!(
                !r.is_single_cardinality(),
                "{} should be multi-cardinality",
                r.display_label()
            );
        }
    }
}
