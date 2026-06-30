use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::shared::detect_default_locale;

use super::Preferences;

const APP_DIR_NAME: &str = "local-game-editor";
const FILE_NAME: &str = "preferences.yml";

pub trait PreferencesRepository: Send + Sync {
    /// 保存済みの Preferences をロードする。
    ///
    /// ファイルが存在しない、または parse に失敗した場合は `Preferences::default()` を返す
    /// （fail-soft、起動を妨げない）。
    fn load(&self) -> Result<Preferences>;

    /// Preferences を保存する。親ディレクトリが無い場合は作成する。
    fn save(&self, preferences: &Preferences) -> Result<()>;
}

pub struct InMemoryPreferencesRepository {
    storage: RwLock<Preferences>,
}

impl InMemoryPreferencesRepository {
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(Preferences::default()),
        }
    }
}

impl Default for InMemoryPreferencesRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl PreferencesRepository for InMemoryPreferencesRepository {
    fn load(&self) -> Result<Preferences> {
        Ok(self.storage.read().expect("RwLock poisoned").clone())
    }

    fn save(&self, preferences: &Preferences) -> Result<()> {
        *self.storage.write().expect("RwLock poisoned") = preferences.clone();
        Ok(())
    }
}

pub struct FilesystemPreferencesRepository {
    path: PathBuf,
}

impl FilesystemPreferencesRepository {
    /// OS 標準のユーザー設定ディレクトリ配下に preferences.yml を割り当てる。
    /// `dirs::config_dir()` が None を返す環境では error。
    pub fn new() -> Result<Self> {
        let base = dirs::config_dir().context(
            "ユーザー設定ディレクトリの取得に失敗しました（dirs::config_dir() が None）",
        )?;
        Ok(Self {
            path: base.join(APP_DIR_NAME).join(FILE_NAME),
        })
    }

    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl PreferencesRepository for FilesystemPreferencesRepository {
    fn load(&self) -> Result<Preferences> {
        if !self.path.exists() {
            // 初回起動: OS locale から推定。既存ユーザーには影響しない (下の serde 経路で
            // ファイル存在時は `Locale::default()` = Ja に固定される)。
            return Ok(Preferences {
                locale: detect_default_locale(),
                ..Preferences::default()
            });
        }
        let content = match fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    "preferences.yml の読み込みに失敗: {} (default にフォールバック)",
                    e
                );
                return Ok(Preferences::default());
            }
        };
        match serde_saphyr::from_str::<Preferences>(&content) {
            Ok(p) => Ok(p),
            Err(e) => {
                tracing::warn!(
                    "preferences.yml の parse に失敗: {} (default にフォールバック)",
                    e
                );
                Ok(Preferences::default())
            }
        }
    }

    fn save(&self, preferences: &Preferences) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let yaml = serde_saphyr::to_string(preferences)?;
        fs::write(&self.path, yaml)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::Theme;
    use super::*;

    #[test]
    fn in_memory_save_and_load() -> Result<()> {
        let repo = InMemoryPreferencesRepository::new();
        // 初期は default
        assert_eq!(repo.load()?, Preferences::default());

        let prefs = Preferences {
            theme: Theme::Dark,
            ..Preferences::default()
        };
        repo.save(&prefs)?;
        assert_eq!(repo.load()?, prefs);
        Ok(())
    }

    #[test]
    fn filesystem_save_and_load_round_trips() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("subdir/preferences.yml");
        let repo = FilesystemPreferencesRepository::from_path(path.clone());

        let prefs = Preferences {
            theme: Theme::Dark,
            ..Preferences::default()
        };
        repo.save(&prefs)?;
        assert!(path.exists(), "save should create the file");
        assert_eq!(repo.load()?, prefs);
        Ok(())
    }

    #[test]
    fn filesystem_load_returns_default_when_file_missing() -> Result<()> {
        // ファイル不在時は OS locale 検出が走るので locale 以外を default と比較する (ADR-0042)。
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("does-not-exist.yml");
        let repo = FilesystemPreferencesRepository::from_path(path);
        let loaded = repo.load()?;
        let expected = Preferences {
            locale: loaded.locale,
            ..Preferences::default()
        };
        assert_eq!(loaded, expected);
        Ok(())
    }

    #[test]
    fn filesystem_load_fills_in_defaults_for_missing_fields() -> Result<()> {
        // 古い preferences.yml (key_bindings セクションが無い) を読んでも、
        // `#[serde(default)]` で `KeyBindings::default()` (Ctrl+S → Save) が補完されること。
        use crate::entities::keybinding::{Action, KeyBindings};

        let dir = tempfile::tempdir()?;
        let path = dir.path().join("preferences.yml");
        fs::write(&path, "theme: dark\n")?;
        let repo = FilesystemPreferencesRepository::from_path(path);
        let loaded = repo.load()?;
        assert_eq!(loaded.theme, Theme::Dark);
        // key_bindings はデフォルト (Ctrl+S → Save) で埋まる
        let default = KeyBindings::default();
        assert_eq!(
            loaded.key_bindings.get(Action::Save),
            default.get(Action::Save),
        );
        Ok(())
    }

    #[test]
    fn filesystem_load_returns_default_when_yaml_is_broken() -> Result<()> {
        // fail-soft: 壊れた yml でも default を返してアプリは起動できる
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("preferences.yml");
        fs::write(&path, "this is :: not a valid yaml :: at all")?;
        let repo = FilesystemPreferencesRepository::from_path(path);
        assert_eq!(repo.load()?, Preferences::default());
        Ok(())
    }
}
