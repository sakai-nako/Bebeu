//! Character の features (FSD: Feature slice)。
//!
//! Character (player / 将来 opponent) の操作・描画・状態管理に関わる system を
//! 1 slice に集約する。editor の `entities/character/` と同じドメイン軸で
//! slicing し、技術名 (animation / movement / state_machine 等) は slice 内の
//! ファイル分割にとどめる (segment 規約からは外れるが、ECS の system 分類として
//! 実用的)。
mod animation;
mod attack;
mod hit_stop;
mod hitbox_debug;
mod movement;
mod state_machine;
// 将来の追加先 (雛形のみ。中身が育ったら個別に pub use を増やす)。
mod ai;

pub use animation::{
    AnimationFrames, AnimationPlugin, AnimationSet, FrameRender, VSYNC_TICK, VSYNC_TICK_SECS,
};
pub use attack::{
    AttackBox, AttackHitConsumed, AttackPlugin, BodyBox, CharacterDepth, HitPoints, aabb_intersects,
};
pub use hit_stop::{HitStopPlugin, HitStopState};
pub use hitbox_debug::{HitboxDebugEnabled, HitboxDebugPlugin};
pub use movement::{
    Enemy, Facing, MainCamera, MovementPlugin, Player, WorldPosition, flip_anchor, total_flip_x,
};
pub use state_machine::{
    AnimationData, CharacterState, EnemyAnimationSet, PlayerAnimationLibrary, StateMachinePlugin,
};
