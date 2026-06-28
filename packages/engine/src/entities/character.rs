//! Character 集約 (FSD: Entity slice)。
//!
//! 1 つの slice 内に全 type (Character / Physics / Role / SpriteGroup / SpriteEntry /
//! Animation / Frame / Layer) を `model.rs` に集約し、loader (Character /
//! Animation / SpriteGroup の `load_from_file`) を `api.rs` に集約する。
//! editor 側の同名スライスと同じ FSD 規約 (slice 直下に segment、サブスライスは作らない)。
mod api;
mod model;
pub use model::{
    AiConfig, AllyConfig, Animation, AttackBox, AttackBoxMeta, AttackBoxOverride, BotConfig,
    Character, DEFAULT_BOUNCE_COUNT, DEFAULT_BOUNCE_DAMPENING, DEFAULT_DEPTH, DEFAULT_GRAVITY,
    DEFAULT_GROUND_FRICTION, DEFAULT_HIT_RECOVERY_MS, DEFAULT_HP, DEFAULT_JUMP_VELOCITY_Y,
    DEFAULT_KNOCKBACK_THRESHOLD, DEFAULT_LIE_DOWN_DURATION_MS, DEFAULT_RISE_DURATION_MS,
    EngagementConfig, Frame, FrameSound, HitBox, HitStop, KnockbackVec, Layer, MeleeConfig,
    Physics, Role, Sound, SoundGroup, SpriteEntry, SpriteGroup, TargetSelector,
};
