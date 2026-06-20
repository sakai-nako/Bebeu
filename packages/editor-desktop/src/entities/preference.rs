mod model;
pub use model::{Preferences, Theme};

mod api;
pub use api::{
    FilesystemPreferencesRepository, InMemoryPreferencesRepository, PreferencesRepository,
};

mod provider;
pub use provider::{use_preferences, use_preferences_provider};
