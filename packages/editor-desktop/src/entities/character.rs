mod model;
pub use model::{
    Animation, BoxKind, Character, CharacterPhysics, DEFAULT_CHARACTER_DEPTH, Frame, FrameSound,
    Layer, SelectedBox, Sound, SoundGroup, Sprite, SpriteGroup,
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
