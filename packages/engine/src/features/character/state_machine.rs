//! Character state machine (FSD: feature slice)。
//!
//! - [`CharacterState`] Component がキャラの現状態 (Idle / Walk / Attack / Hit) を保持する。
//!   Player / Enemy で同じ enum を使い、`is_locked` な状態 (Attack / Hit) は入力 / AI
//!   による上書きを受け付けない。
//! - Player の Animation は [`PlayerAnimationLibrary`] Resource に role ごとに pre-build
//!   して入れておき、[`sync_animation`] が `Changed<CharacterState>` で hot swap する。
//! - Enemy の Animation は entity 自身が [`EnemyAnimationSet`] component で持ち、
//!   [`sync_enemy_animation`] が同様に hot swap する (将来複数キャラ対応のため Resource
//!   ではなく component に持たせている)。
//! - 1-shot 状態 (Attack / Hit など `is_locked` の Animation) は [`end_oneshot_actions`]
//!   が再生終端で Idle に戻す。
//!
//! state の更新 (入力に応じた Idle ⇄ Walk) は [`super::movement::handle_input`] が行う。
use std::collections::HashMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::entities::character::Role;

use super::animation::{AnimationFrames, FrameRender};
use super::debug_control::SimulationSet;
use super::hit_stop::HitStopState;
use super::knockback::{Combatant, FinalAction};
use super::movement::{Enemy, Facing, Player, flip_anchor, total_flip_x};

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharacterState {
    #[default]
    Idle,
    Walk,
    Attack,
    Hit,
    /// 吹っ飛び上昇 (ADR-0024)。`KinematicVel` の積分 + gravity を [`super::knockback`]
    /// が回し、vel_y<=0 (apex) で [`Self::KnockbackDown`] に遷移する。
    KnockbackUp,
    /// 吹っ飛び下降。着地 (y<=0) で `Combatant.remaining_bounces` に応じて
    /// [`Self::BounceUp`] (残あり) か [`Self::Slide`] (残ゼロ) に遷移。
    KnockbackDown,
    /// バウンド後の上昇。`KnockbackUp` と同じ物理だが、着地時の挙動と意味づけが違うので
    /// Action 別。apex で [`Self::BounceDown`] へ。
    BounceUp,
    /// バウンド後の下降。`KnockbackDown` と同じ物理。着地で残数を 1 消費して再度 Bounce
    /// するか、ゼロなら [`Self::Slide`] に落ちる。
    BounceDown,
    /// 地面を滑る状態。`Physics.ground_friction` で X/Z を減速し、ほぼ停止で
    /// [`Self::LieDown`] に進む。Phase B の最終フェーズ。
    Slide,
    /// 着地後の倒れポーズ。`Combatant.stage_timer_ticks` で固定時間カウントし、終わったら
    /// [`Self::Rise`] へ。Animation 終端でも進む (二重終了条件、ADR-0025)。
    /// `Combatant.final_action == Dead` なら Rise に遷移せず永続停止 (= KO 演出)。
    LieDown,
    /// 起き上がりポーズ。終わったら [`Self::Idle`] に戻る。
    Rise,
    /// **Down 中 (Slide / LieDown / Rise) に hit を受けた**ときに遷移する地上 hit。
    /// 通常 [`Self::Hit`] は立ちポーズなので、地面に伏せている状態の hit には不適切。
    /// Animation 終端で [`Self::LieDown`] に戻り、`stage_timer` を fresh にリセット (=
    /// 倒れたまま、down 時間が延長される)。`advance_stage_timer` が遷移を担当。
    DownHit,
    /// 下段攻撃 (足元の AttackBox)。倒れた敵 (LieDown body box は世界 Y 0-14) に当てる
    /// ための攻撃モード。`is_attack_hit_active` / `resolve_hits` は `Attack` と同等扱い。
    /// `end_oneshot_actions` が再生終端で Idle に戻す。
    DownAttack,
}

impl CharacterState {
    /// Animation 解決用 Role。Phase A/B は 1:1 mapping のみ。ADR-0025 の prefix 付き
    /// fallback (`back_*` / `dead_*`) は Phase C で導入。
    #[must_use]
    pub fn to_role(self) -> Role {
        match self {
            Self::Idle => Role::Idle,
            Self::Walk => Role::Walk,
            Self::Attack => Role::Attack,
            Self::Hit => Role::Hit,
            Self::KnockbackUp => Role::KnockbackUp,
            Self::KnockbackDown => Role::KnockbackDown,
            Self::BounceUp => Role::BounceUp,
            Self::BounceDown => Role::BounceDown,
            Self::Slide => Role::Slide,
            Self::LieDown => Role::LieDown,
            Self::Rise => Role::Rise,
            Self::DownHit => Role::DownHit,
            Self::DownAttack => Role::DownAttack,
        }
    }

    /// Attack / Hit / 吹っ飛びフロー中は、入力 / AI による Idle/Walk 等への上書きを抑制する。
    /// 単発系 (Attack / Hit / Rise) は再生終端で `end_oneshot_actions` が Idle に戻し、
    /// Knockback 物理系 (KnockbackUp/Down / Bounce / Slide / LieDown) は
    /// `super::knockback` が次段へ遷移させる。
    #[must_use]
    pub fn is_locked(self) -> bool {
        matches!(
            self,
            Self::Attack
                | Self::Hit
                | Self::KnockbackUp
                | Self::KnockbackDown
                | Self::BounceUp
                | Self::BounceDown
                | Self::Slide
                | Self::LieDown
                | Self::Rise
                | Self::DownHit
                | Self::DownAttack
        )
    }
}

/// 1 役割ぶんの描画データ (battle 起動時にビルドして cache する)。
#[derive(Debug, Clone)]
pub struct AnimationData {
    pub frames: Vec<FrameRender>,
    pub is_loop: bool,
    pub loop_start_index: usize,
}

/// role ごとに cache した AnimationData の library。battle setup で insert される。
#[derive(Resource, Default)]
pub struct PlayerAnimationLibrary {
    by_role: HashMap<Role, AnimationData>,
}

impl PlayerAnimationLibrary {
    pub fn insert(&mut self, role: Role, data: AnimationData) {
        self.by_role.insert(role, data);
    }

    #[must_use]
    pub fn get(&self, role: Role) -> Option<&AnimationData> {
        self.by_role.get(&role)
    }
}

pub struct StateMachinePlugin;

impl Plugin for StateMachinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerAnimationLibrary>().add_systems(
            Update,
            (end_oneshot_actions, sync_animation, sync_enemy_animation)
                .in_set(SimulationSet::Active),
        );
    }
}

/// 単発 (locked) アクションの Animation が末尾 frame を消化したら Idle に戻す。
/// Player / Enemy 両方の `CharacterState` に対して動き、`Changed<CharacterState>` を
/// トリガして対応する sync_* system が AnimationFrames を差し替える。
/// `HitStopState` 中は Animation 進行が freeze されているので state 遷移も block する
/// (= hit_stop 解除後に通常の Idle 復帰へ流れる)。
///
/// 吹っ飛びフロー (`KnockbackUp`/`Down`/`LieDown`/`Rise`) は state 遷移が `super::knockback`
/// (物理 / 固定 timer) 側で起きるので、この generic な末尾 → Idle は適用しない。
/// `Attack` / `Hit` のみが対象。
fn end_oneshot_actions(
    mut query: Query<(&AnimationFrames, &mut CharacterState), Without<HitStopState>>,
) {
    for (anim, mut state) in &mut query {
        if matches!(
            *state,
            CharacterState::Attack | CharacterState::Hit | CharacterState::DownAttack
        ) && anim.is_finished()
        {
            *state = CharacterState::Idle;
        }
    }
}

/// ADR-0025: 被弾方向 (`hit_from_behind`) と致命傷 (`final_action`) で Animation を多段
/// フォールバックする。優先度は:
///
/// ```text
/// (Dead, Behind) → DeadBackX → DeadX → BackX → X
/// (Dead, Front)  →            DeadX →         X
/// (Alive, Behind) →                  BackX → X
/// (Alive, Front)  →                          X
/// ```
///
/// ここで `X` は `state.to_role()` の基本 Role。Knockback 系以外 (Idle/Walk/Attack/Hit/...)
/// は prefix variant が存在しないので、結果として 1:1 mapping になる。
///
/// 最終 safety net として `is_knockback_role(X) → Hit` のフォールバックも残す
/// (= 旧 character が Hit しか持たないケースを保護)。
fn resolve_animation_role<'a, F>(
    state: CharacterState,
    hit_from_behind: bool,
    final_action: FinalAction,
    get: F,
) -> Option<&'a AnimationData>
where
    F: Fn(Role) -> Option<&'a AnimationData>,
{
    let base = state.to_role();
    let dead = final_action == FinalAction::Dead;
    let back = hit_from_behind;

    if dead && back {
        if let Some(data) = dead_back_variant(base).and_then(&get) {
            return Some(data);
        }
    }
    if dead {
        if let Some(data) = dead_variant(base).and_then(&get) {
            return Some(data);
        }
    }
    if back {
        if let Some(data) = back_variant(base).and_then(&get) {
            return Some(data);
        }
    }
    if let Some(data) = get(base) {
        return Some(data);
    }
    // 最終 safety net: Knockback 系 (= 物理ステージ) の Role が character に 1 つも
    // 登録されていない場合、`Hit` まで劣化させる。Phase A/B 時代の暫定処理を継承。
    if is_knockback_role(base) {
        return get(Role::Hit);
    }
    None
}

/// `role` が ADR-0024/0025 の物理ステージ (= Knockback フロー 7 個 + prefix variants) なら
/// `true`。Animation の最終 safety net 判定で使う。
#[must_use]
fn is_knockback_role(role: Role) -> bool {
    matches!(
        role,
        Role::KnockbackUp
            | Role::KnockbackDown
            | Role::BounceUp
            | Role::BounceDown
            | Role::Slide
            | Role::LieDown
            | Role::Rise
            | Role::DownHit
    )
}

/// `back_*` 系 Role への変換。Rise も BackRise を持つ (Dead 系のみ Rise なし)。
#[must_use]
fn back_variant(role: Role) -> Option<Role> {
    Some(match role {
        Role::KnockbackUp => Role::BackKnockbackUp,
        Role::KnockbackDown => Role::BackKnockbackDown,
        Role::BounceUp => Role::BackBounceUp,
        Role::BounceDown => Role::BackBounceDown,
        Role::Slide => Role::BackSlide,
        Role::LieDown => Role::BackLieDown,
        Role::Rise => Role::BackRise,
        _ => return None,
    })
}

/// `dead_*` 系 Role への変換。Rise / BackRise は Dead 系の対応 variant を持たない (死んだら
/// 起き上がらないため)。state 側のフローでも Dead 中は Rise に進まないので呼ばれないはず。
#[must_use]
fn dead_variant(role: Role) -> Option<Role> {
    Some(match role {
        Role::KnockbackUp => Role::DeadKnockbackUp,
        Role::KnockbackDown => Role::DeadKnockbackDown,
        Role::BounceUp => Role::DeadBounceUp,
        Role::BounceDown => Role::DeadBounceDown,
        Role::Slide => Role::DeadSlide,
        Role::LieDown => Role::DeadLieDown,
        _ => return None,
    })
}

/// `dead_back_*` 系 Role への変換。Rise 系は持たない (Dead 系と同じ理由)。
#[must_use]
fn dead_back_variant(role: Role) -> Option<Role> {
    Some(match role {
        Role::KnockbackUp => Role::DeadBackKnockbackUp,
        Role::KnockbackDown => Role::DeadBackKnockbackDown,
        Role::BounceUp => Role::DeadBackBounceUp,
        Role::BounceDown => Role::DeadBackBounceDown,
        Role::Slide => Role::DeadBackSlide,
        Role::LieDown => Role::DeadBackLieDown,
        _ => return None,
    })
}

/// Player の CharacterState が変化したら、`PlayerAnimationLibrary` (Resource) から該当
/// role の AnimationData を引いて AnimationFrames / Sprite.image / Sprite.flip_x / Anchor
/// を **同 frame で一括更新**する。
///
/// `commands.entity().insert()` で AnimationFrames を遅延入れ替えすると、同 frame の
/// 描画では「新しい sprite.image だが古い anchor/flip_x」の状態が見えてちらつく。
/// 直接 `&mut` で書き換えることで同 frame 内で整合させる。
fn sync_animation(
    library: Res<PlayerAnimationLibrary>,
    mut query: Query<
        (
            &CharacterState,
            &Facing,
            &Combatant,
            &mut AnimationFrames,
            &mut Sprite,
            &mut Anchor,
        ),
        (With<Player>, Changed<CharacterState>),
    >,
) {
    for (state, facing, combatant, mut anim, mut sprite, mut anchor) in &mut query {
        let Some(data) = resolve_animation_role(
            *state,
            combatant.hit_from_behind,
            combatant.final_action,
            |r| library.get(r),
        ) else {
            tracing::warn!(
                ?state,
                base_role = ?state.to_role(),
                hit_from_behind = combatant.hit_from_behind,
                final_action = ?combatant.final_action,
                "state_machine: no AnimationData for player",
            );
            continue;
        };
        apply_animation(data, *facing, &mut anim, &mut sprite, &mut anchor);
    }
}

/// Enemy の CharacterState が変化したら、entity 自身が持つ `EnemyAnimationSet` から
/// AnimationData を引いて同様に hot swap する。Enemy 用 library は entity 持ちのため
/// 将来複数 character (`opponent_triggers`) で別 role 集合を持たせやすい。
fn sync_enemy_animation(
    mut query: Query<
        (
            &CharacterState,
            &Facing,
            &Combatant,
            &EnemyAnimationSet,
            &mut AnimationFrames,
            &mut Sprite,
            &mut Anchor,
        ),
        (With<Enemy>, Changed<CharacterState>),
    >,
) {
    for (state, facing, combatant, set, mut anim, mut sprite, mut anchor) in &mut query {
        let Some(data) = resolve_animation_role(
            *state,
            combatant.hit_from_behind,
            combatant.final_action,
            |r| set.get(r),
        ) else {
            tracing::warn!(
                ?state,
                base_role = ?state.to_role(),
                hit_from_behind = combatant.hit_from_behind,
                final_action = ?combatant.final_action,
                "state_machine: no AnimationData for enemy",
            );
            continue;
        };
        apply_animation(data, *facing, &mut anim, &mut sprite, &mut anchor);
    }
}

/// `AnimationData` を `AnimationFrames` / `Sprite` / `Anchor` に同 frame で焼き込む。
/// Player / Enemy 両方の sync 系統から呼ぶ共有 path。
fn apply_animation(
    data: &AnimationData,
    facing: Facing,
    anim: &mut AnimationFrames,
    sprite: &mut Sprite,
    anchor: &mut Anchor,
) {
    *anim = AnimationFrames::new(data.frames.clone(), data.is_loop, data.loop_start_index);
    if let Some(first) = data.frames.first() {
        sprite.image = first.handle.clone();
        let flip = total_flip_x(facing, first.flip_x);
        sprite.flip_x = flip;
        *anchor = flip_anchor(first.anchor, flip);
    }
}

/// Enemy が entity ローカルに持つ role → AnimationData の小さな library。
/// 現状 Idle / Hit を入れる想定だが、将来 Walk / Attack 等が増えても同じ形で拡張できる。
#[derive(Component, Debug, Clone, Default)]
pub struct EnemyAnimationSet {
    by_role: HashMap<Role, AnimationData>,
}

impl EnemyAnimationSet {
    pub fn insert(&mut self, role: Role, data: AnimationData) {
        self.by_role.insert(role, data);
    }

    #[must_use]
    pub fn get(&self, role: Role) -> Option<&AnimationData> {
        self.by_role.get(&role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_state_default_is_idle() {
        assert_eq!(CharacterState::default(), CharacterState::Idle);
    }

    #[test]
    fn character_state_to_role_maps_correctly() {
        assert_eq!(CharacterState::Idle.to_role(), Role::Idle);
        assert_eq!(CharacterState::Walk.to_role(), Role::Walk);
        assert_eq!(CharacterState::Attack.to_role(), Role::Attack);
        assert_eq!(CharacterState::Hit.to_role(), Role::Hit);
        assert_eq!(CharacterState::KnockbackUp.to_role(), Role::KnockbackUp);
        assert_eq!(CharacterState::KnockbackDown.to_role(), Role::KnockbackDown);
        assert_eq!(CharacterState::BounceUp.to_role(), Role::BounceUp);
        assert_eq!(CharacterState::BounceDown.to_role(), Role::BounceDown);
        assert_eq!(CharacterState::Slide.to_role(), Role::Slide);
        assert_eq!(CharacterState::LieDown.to_role(), Role::LieDown);
        assert_eq!(CharacterState::Rise.to_role(), Role::Rise);
        assert_eq!(CharacterState::DownHit.to_role(), Role::DownHit);
    }

    #[test]
    fn character_state_is_locked_for_oneshot_actions() {
        // 入力 / AI で上書きされない state: Attack / Hit / 吹っ飛びフロー全部。
        assert!(!CharacterState::Idle.is_locked());
        assert!(!CharacterState::Walk.is_locked());
        assert!(CharacterState::Attack.is_locked());
        assert!(CharacterState::Hit.is_locked());
        assert!(CharacterState::KnockbackUp.is_locked());
        assert!(CharacterState::KnockbackDown.is_locked());
        assert!(CharacterState::BounceUp.is_locked());
        assert!(CharacterState::BounceDown.is_locked());
        assert!(CharacterState::Slide.is_locked());
        assert!(CharacterState::LieDown.is_locked());
        assert!(CharacterState::Rise.is_locked());
        assert!(CharacterState::DownHit.is_locked());
    }

    /// id 別に AnimationData を作る。`loop_start_index` を識別子として使い、
    /// `resolve_animation_role` の戻り値がどの Role 経路で解決されたかを比較する。
    fn anim_with_id(id: usize) -> AnimationData {
        AnimationData {
            frames: vec![],
            is_loop: false,
            loop_start_index: id,
        }
    }

    fn dummy_anim() -> AnimationData {
        anim_with_id(0)
    }

    #[test]
    fn resolve_animation_role_returns_specific_when_registered() {
        let data = dummy_anim();
        let get = |r: Role| -> Option<&AnimationData> { (r == Role::KnockbackUp).then_some(&data) };
        assert!(
            resolve_animation_role(
                CharacterState::KnockbackUp,
                false,
                FinalAction::LieDown,
                get,
            )
            .is_some()
        );
    }

    #[test]
    fn resolve_animation_role_prefers_dead_back_over_dead_over_back_over_base() {
        // 全 4 variant が登録されているケース: (Dead, Behind) は DeadBack を返す。
        let dead_back = anim_with_id(4);
        let dead = anim_with_id(3);
        let back = anim_with_id(2);
        let base = anim_with_id(1);
        let get = |r: Role| -> Option<&AnimationData> {
            Some(match r {
                Role::DeadBackKnockbackUp => &dead_back,
                Role::DeadKnockbackUp => &dead,
                Role::BackKnockbackUp => &back,
                Role::KnockbackUp => &base,
                _ => return None,
            })
        };
        // (Dead, Behind) → DeadBack 優先
        let data =
            resolve_animation_role(CharacterState::KnockbackUp, true, FinalAction::Dead, get)
                .expect("should resolve");
        assert_eq!(data.loop_start_index, 4);
    }

    #[test]
    fn resolve_animation_role_falls_back_through_chain() {
        // dead_back が未登録なら Dead に劣化。
        let dead = anim_with_id(3);
        let back = anim_with_id(2);
        let base = anim_with_id(1);
        let get = |r: Role| -> Option<&AnimationData> {
            Some(match r {
                Role::DeadKnockbackUp => &dead,
                Role::BackKnockbackUp => &back,
                Role::KnockbackUp => &base,
                _ => return None,
            })
        };
        let data =
            resolve_animation_role(CharacterState::KnockbackUp, true, FinalAction::Dead, get)
                .expect("should resolve");
        // DeadBackKnockbackUp が無いので Dead に劣化
        assert_eq!(data.loop_start_index, 3);
    }

    #[test]
    fn resolve_animation_role_alive_front_uses_base_only() {
        // (Alive, Front) は Back / Dead variant をスキップして直接 base に行く。
        let back = anim_with_id(2);
        let base = anim_with_id(1);
        let get = |r: Role| -> Option<&AnimationData> {
            Some(match r {
                Role::BackKnockbackUp => &back,
                Role::KnockbackUp => &base,
                _ => return None,
            })
        };
        let data = resolve_animation_role(
            CharacterState::KnockbackUp,
            false,
            FinalAction::LieDown,
            get,
        )
        .expect("should resolve");
        assert_eq!(data.loop_start_index, 1);
    }

    #[test]
    fn resolve_animation_role_safety_net_falls_back_to_hit() {
        // base も prefix variant も全く未登録だが Hit はある → 物理ステージ Role なら Hit。
        let hit = dummy_anim();
        let get = |r: Role| -> Option<&AnimationData> { (r == Role::Hit).then_some(&hit) };
        for state in [
            CharacterState::KnockbackUp,
            CharacterState::KnockbackDown,
            CharacterState::BounceUp,
            CharacterState::BounceDown,
            CharacterState::Slide,
            CharacterState::LieDown,
            CharacterState::Rise,
            CharacterState::DownHit,
        ] {
            assert!(
                resolve_animation_role(state, false, FinalAction::LieDown, get).is_some(),
                "{state:?} should fall back to Hit"
            );
        }
    }

    #[test]
    fn resolve_animation_role_idle_walk_do_not_fall_back_to_hit() {
        // Idle / Walk / Attack は物理ステージではないので Hit fallback の対象外。
        let hit = dummy_anim();
        let get = |r: Role| -> Option<&AnimationData> { (r == Role::Hit).then_some(&hit) };
        for state in [
            CharacterState::Idle,
            CharacterState::Walk,
            CharacterState::Attack,
        ] {
            assert!(
                resolve_animation_role(state, false, FinalAction::LieDown, get).is_none(),
                "{state:?} should NOT fall back to Hit"
            );
        }
    }

    #[test]
    fn resolve_animation_role_rise_has_back_variant_but_no_dead() {
        // Rise は BackRise を持つが、Dead 系 Rise variant は無い (dead だと Rise しないため)。
        // dead_variant(Rise) / dead_back_variant(Rise) は None。よって Dead フラグ + Rise の
        // ケースでは Back / Base を試行する。
        assert!(dead_variant(Role::Rise).is_none());
        assert!(dead_back_variant(Role::Rise).is_none());
        assert_eq!(back_variant(Role::Rise), Some(Role::BackRise));
    }

    #[test]
    fn enemy_animation_set_round_trip() {
        let mut set = EnemyAnimationSet::default();
        let data = AnimationData {
            frames: vec![],
            is_loop: false,
            loop_start_index: 0,
        };
        set.insert(Role::Hit, data);
        assert!(set.get(Role::Hit).is_some());
        assert!(set.get(Role::Idle).is_none());
    }

    #[test]
    fn library_insert_and_get_round_trip() {
        let mut lib = PlayerAnimationLibrary::default();
        let data = AnimationData {
            frames: vec![],
            is_loop: true,
            loop_start_index: 0,
        };
        lib.insert(Role::Walk, data);
        assert!(lib.get(Role::Walk).is_some());
        assert!(lib.get(Role::Idle).is_none());
    }
}
