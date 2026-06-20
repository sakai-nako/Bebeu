//! Character state machine (FSD: feature slice)。
//!
//! - [`PlayerState`] Component が player の現状態 (Idle / Walk / ...) を保持する。
//! - [`PlayerAnimationLibrary`] Resource が role ごとに事前構築した [`AnimationData`]
//!   (FrameRender 配列 + ループ情報) を持つ。battle 起動時に一度だけ全 role 分を
//!   組み立てて入れておく。
//! - [`sync_animation`] system は `PlayerState` の `Changed` を検知して、対応する
//!   `AnimationData` から `AnimationFrames` を作り直して entity に attach し直す。
//!
//! state の更新 (入力に応じた Idle ⇄ Walk) は [`super::movement::handle_input`] が行う。
use std::collections::HashMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::entities::character::Role;

use super::animation::{AnimationFrames, FrameRender};
use super::movement::{Facing, Player, flip_anchor, total_flip_x};

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerState {
    #[default]
    Idle,
    Walk,
}

impl PlayerState {
    #[must_use]
    pub fn to_role(self) -> Role {
        match self {
            Self::Idle => Role::Idle,
            Self::Walk => Role::Walk,
        }
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
        app.init_resource::<PlayerAnimationLibrary>()
            .add_systems(Update, sync_animation);
    }
}

/// PlayerState が変化したら、library から該当 role の AnimationData を引いて
/// AnimationFrames / Sprite.image / Sprite.flip_x / Anchor を **同 frame で一括更新**する。
///
/// `commands.entity().insert()` で AnimationFrames を遅延入れ替えすると、同 frame の
/// 描画では「新しい sprite.image だが古い anchor/flip_x」の状態が見えてちらつく。
/// 直接 `&mut` で書き換えることで同 frame 内で整合させる。
fn sync_animation(
    library: Res<PlayerAnimationLibrary>,
    mut query: Query<
        (
            &PlayerState,
            &Facing,
            &mut AnimationFrames,
            &mut Sprite,
            &mut Anchor,
        ),
        (With<Player>, Changed<PlayerState>),
    >,
) {
    for (state, facing, mut anim, mut sprite, mut anchor) in &mut query {
        let role = state.to_role();
        let Some(data) = library.get(role) else {
            tracing::warn!(?state, ?role, "state_machine: no AnimationData for role");
            continue;
        };
        *anim = AnimationFrames::new(
            data.frames.clone(),
            data.is_loop,
            data.loop_start_index,
        );
        if let Some(first) = data.frames.first() {
            sprite.image = first.handle.clone();
            let flip = total_flip_x(*facing, first.flip_x);
            sprite.flip_x = flip;
            *anchor = flip_anchor(first.anchor, flip);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_state_default_is_idle() {
        assert_eq!(PlayerState::default(), PlayerState::Idle);
    }

    #[test]
    fn player_state_to_role_maps_correctly() {
        assert_eq!(PlayerState::Idle.to_role(), Role::Idle);
        assert_eq!(PlayerState::Walk.to_role(), Role::Walk);
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
