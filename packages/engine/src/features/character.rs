//! Character の features (FSD: Feature slice)。
//!
//! Character (player / 将来 opponent) の操作・描画・状態管理に関わる system を
//! 1 slice に集約する。editor の `entities/character/` と同じドメイン軸で
//! slicing し、技術名 (animation / movement / state_machine 等) は slice 内の
//! ファイル分割にとどめる (segment 規約からは外れるが、ECS の system 分類として
//! 実用的)。
mod ai;
mod animation;
mod attack;
mod debug_control;
mod hit_stop;
mod hitbox_debug;
mod knockback;
mod movement;
mod sound;
mod state_debug;
mod state_machine;

pub use ai::{
    AiCommand, AiPlugin, AiSet, AllyBrain, AllyState, BotBrain, Brain, BrainCounters,
    EngagementState, MeleeBrain, apply_command,
};
// MeleeConfig / AllyConfig は entities/character に移したが、features 側でも facade として
// re-export する (battle.rs / ai.rs の既存 import 経路を維持。entities → features の一方向
// 依存は OK)。ADR-0039 で `EngagementConfig` / `TargetSelector` も同様に re-export する。
pub use crate::entities::character::{
    AllyConfig, BotConfig, EngagementConfig, MeleeConfig, TargetSelector,
};
pub use animation::{
    AnimationFrames, AnimationPlugin, AnimationSet, FrameRender, VSYNC_TICK, VSYNC_TICK_SECS,
};
pub use attack::{
    AttackBox, AttackHitConsumed, AttackPlugin, AttackSet, BodyBox, CharacterDepth, HitPoints,
    aabb_intersects,
};
pub use debug_control::{DebugControlPlugin, DebugPause, SimulationSet};
pub use hit_stop::{HitStopPlugin, HitStopState};
pub use hitbox_debug::{HitboxDebugEnabled, HitboxDebugPlugin};
pub use knockback::{Combatant, FinalAction, KinematicVel, KnockbackPlugin, PhysicsParams};
pub use movement::{
    Controller, EnemyTag, Facing, LastEngagedWith, MainCamera, MovementPlugin, Side, WorldPosition,
    flip_anchor, total_flip_x,
};
pub use sound::{
    AttackOutcome, CharacterSounds, SoundDispatch, SoundPlugin, bake_character_sounds,
};
pub use state_debug::{StateDebugEnabled, StateDebugPlugin};
pub use state_machine::{
    AnimationData, CharacterState, EnemyAnimationSet, PlayerAnimationLibrary,
    PlayerSpriteGroupRegistry, PlayerSpriteGroups, StateMachinePlugin,
};
