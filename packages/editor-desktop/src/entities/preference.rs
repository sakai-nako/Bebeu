mod model;
pub use model::{Locale, Preferences, Theme};

mod api;
pub use api::{
    FilesystemPreferencesRepository, InMemoryPreferencesRepository, PreferencesRepository,
};

mod provider;
pub use provider::{use_preferences, use_preferences_provider, use_t, use_t_args};
