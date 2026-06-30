//! 旧 `internal/engine/shared/*` に相当する engine 横断モジュール群。雛形のみ。
pub mod assets;
pub mod audio;
pub mod config;
pub mod flip;
pub mod png_header;
pub mod projection;
pub mod settings;

mod input;
pub use input::{
    Action, ActionMap, MenuAction, key_code_from_str, key_code_to_str, menu_action_just_pressed,
    menu_action_pressed, write_input_yml,
};

mod player_id;
pub use player_id::PlayerId;
