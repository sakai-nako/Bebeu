mod model;
pub use model::{
    AiConfig, AiKind, AllyConfig, Animation, BotConfig, BoxKind, Character, CharacterPhysics,
    DEFAULT_AI_ATTACK_COOLDOWN_TICKS, DEFAULT_AI_ATTACK_ENTER_RANGE_PX,
    DEFAULT_AI_ATTACK_EXIT_RANGE_PX, DEFAULT_AI_CHASE_ENTER_RANGE_PX,
    DEFAULT_AI_CHASE_EXIT_RANGE_PX, DEFAULT_AI_DECISION_INTERVAL_TICKS,
    DEFAULT_AI_FOLLOW_DISTANCE_MAX_PX, DEFAULT_AI_FOLLOW_DISTANCE_MIN_PX,
    DEFAULT_AI_MIN_DWELL_TICKS, DEFAULT_CHARACTER_DEPTH, EngagementConfig, Frame, FrameSound,
    Layer, MeleeConfig, SelectedBox, Sound, SoundGroup, Sprite, SpriteGroup, TargetSelector,
};

mod role;
pub use role::{
    Role, RoleViolation, Severity, TerminatorKind, validate_animations, validate_for_save,
};

mod api;
pub use api::{
    CharacterRepository, FilesystemCharacterRepository, ImportOutcome, InMemoryCharacterRepository,
};

mod refresh;
pub use refresh::{
    CharactersRefreshTrigger, use_characters_refresh, use_characters_refresh_provider,
};

mod playback;
pub use playback::{
    PlaybackConfig, PlaybackState, new_cancel_token, spawn_playback_thread, use_playback,
    use_playback_provider,
};
