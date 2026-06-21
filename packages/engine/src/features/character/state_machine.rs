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
use super::hit_stop::HitStopState;
use super::movement::{Enemy, Facing, Player, flip_anchor, total_flip_x};

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharacterState {
    #[default]
    Idle,
    Walk,
    Attack,
    Hit,
}

impl CharacterState {
    #[must_use]
    pub fn to_role(self) -> Role {
        match self {
            Self::Idle => Role::Idle,
            Self::Walk => Role::Walk,
            Self::Attack => Role::Attack,
            Self::Hit => Role::Hit,
        }
    }

    /// Attack / Hit のような単発 (is_loop=false 想定) アクション中は、入力 / AI による
    /// Idle/Walk 等への上書きを抑制する。終端は [`end_oneshot_actions`] が Idle に戻す。
    #[must_use]
    pub fn is_locked(self) -> bool {
        matches!(self, Self::Attack | Self::Hit)
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
            (end_oneshot_actions, sync_animation, sync_enemy_animation),
        );
    }
}

/// 単発 (locked) アクションの Animation が末尾 frame を消化したら Idle に戻す。
/// Player / Enemy 両方の `CharacterState` に対して動き、`Changed<CharacterState>` を
/// トリガして対応する sync_* system が AnimationFrames を差し替える。
/// `HitStopState` 中は Animation 進行が freeze されているので state 遷移も block する
/// (= hit_stop 解除後に通常の Idle 復帰へ流れる)。
fn end_oneshot_actions(
    mut query: Query<(&AnimationFrames, &mut CharacterState), Without<HitStopState>>,
) {
    for (anim, mut state) in &mut query {
        if state.is_locked() && anim.is_finished() {
            *state = CharacterState::Idle;
        }
    }
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
            &mut AnimationFrames,
            &mut Sprite,
            &mut Anchor,
        ),
        (With<Player>, Changed<CharacterState>),
    >,
) {
    for (state, facing, mut anim, mut sprite, mut anchor) in &mut query {
        let role = state.to_role();
        let Some(data) = library.get(role) else {
            tracing::warn!(
                ?state,
                ?role,
                "state_machine: no AnimationData for player role"
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
            &EnemyAnimationSet,
            &mut AnimationFrames,
            &mut Sprite,
            &mut Anchor,
        ),
        (With<Enemy>, Changed<CharacterState>),
    >,
) {
    for (state, facing, set, mut anim, mut sprite, mut anchor) in &mut query {
        let role = state.to_role();
        let Some(data) = set.get(role) else {
            tracing::warn!(
                ?state,
                ?role,
                "state_machine: no AnimationData for enemy role"
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
    }

    #[test]
    fn character_state_is_locked_for_oneshot_actions() {
        // Attack / Hit は 1-shot で、入力 / AI からは上書きされない。
        assert!(!CharacterState::Idle.is_locked());
        assert!(!CharacterState::Walk.is_locked());
        assert!(CharacterState::Attack.is_locked());
        assert!(CharacterState::Hit.is_locked());
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
