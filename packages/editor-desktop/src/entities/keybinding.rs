mod model;
pub use model::{Action, KeyBindings};

mod dispatch;
pub use dispatch::{
    KeyboardActionDispatcher, KeyboardActionRequest, use_keyboard_action_dispatcher,
    use_keyboard_action_provider,
};
