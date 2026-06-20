use anyhow::{Result, anyhow, bail};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::Level;

const LEVELS_DIR: &str = "levels";
const FILE_EXT: &str = "yml";

pub trait LevelRepository: Send + Sync {
    /// 名前指定で Level をロードする。
    ///
    /// ファイルが存在しない、または parse に失敗した場合は `Level::with_defaults(name)` を返す
    /// (fail-soft、editor で未保存の Level でも開けるよう)。engine 起動時や Project 詳細
    /// ページの一覧用途で使う。「ファイルが本当に存在するか」を区別したい editor 側の
    /// 詳細ページなどでは `get` を使うこと。
    fn load(&self, name: &str) -> Result<Level>;

    /// 名前指定で Level をロードする。ファイルが存在しなければ `Ok(None)` を返す
    /// (Character の `CharacterRepository::get` と対称)。詳細編集ページ用。
    fn get(&self, name: &str) -> Result<Option<Level>>;

    /// 既存 Level の名前一覧を返す (ソート済み)。
    fn list(&self) -> Result<Vec<String>>;

    /// Level を新規作成する。同名が既に存在する場合はエラー。
    fn create(&self, level: &Level) -> Result<()>;

    /// Level を保存する (上書き)。親ディレクトリが無い場合は作成する。
    fn save(&self, level: &Level) -> Result<()>;

    /// Level の YAML ファイル名を rename する。
    /// `old` が存在しない / `new` が既に存在する場合はエラー。
    /// **注意**: Project YAML 側の `levels[]` 参照は更新しない (master pool の独立操作)。
    fn rename(&self, old: &str, new: &str) -> Result<()>;

    /// Level を削除する。存在しない場合はエラー。
    /// **注意**: Project YAML 側の `levels[]` 参照は更新しない。
    fn delete(&self, name: &str) -> Result<()>;

    /// 外部画像を `{workspace}/data/levels/{level_name}/base.{ext}` にコピーし、
    /// `Level.base` に書く相対ファイル名 (`base.{ext}`) を返す。同名ファイルは上書き。
    /// 拡張子は source のものを小文字化して保持する。
    fn import_base_image(&self, level_name: &str, source: &Path) -> Result<String>;

    /// `import_base_image` で生成したファイルを削除する (create 失敗時のロールバック用)。
    /// 不在のときは no-op。
    fn delete_base_image(&self, level_name: &str, basename: &str) -> Result<()>;

    /// 指定名の Level が存在するか。Create / Rename の事前重複チェック用。
    fn exists(&self, name: &str) -> Result<bool> {
        Ok(self.list()?.iter().any(|n| n == name))
    }
}

pub struct InMemoryLevelRepository {
    storage: RwLock<HashMap<String, Level>>,
}

impl InMemoryLevelRepository {
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryLevelRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelRepository for InMemoryLevelRepository {
    fn load(&self, name: &str) -> Result<Level> {
        let map = self.storage.read().expect("RwLock poisoned");
        Ok(map
            .get(name)
            .cloned()
            .unwrap_or_else(|| Level::with_defaults(name)))
    }

    fn get(&self, name: &str) -> Result<Option<Level>> {
        let map = self.storage.read().expect("RwLock poisoned");
        Ok(map.get(name).cloned())
    }

    fn list(&self) -> Result<Vec<String>> {
        let map = self.storage.read().expect("RwLock poisoned");
        let mut names: Vec<String> = map.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    fn create(&self, level: &Level) -> Result<()> {
        let mut map = self.storage.write().expect("RwLock poisoned");
        if map.contains_key(&level.name) {
            bail!("Level '{}' は既に存在します", level.name);
        }
        map.insert(level.name.clone(), level.clone());
        Ok(())
    }

    fn save(&self, level: &Level) -> Result<()> {
        let mut map = self.storage.write().expect("RwLock poisoned");
        map.insert(level.name.clone(), level.clone());
        Ok(())
    }

    fn rename(&self, old: &str, new: &str) -> Result<()> {
        let mut map = self.storage.write().expect("RwLock poisoned");
        if old == new {
            bail!("rename: 旧名と新名が同じです");
        }
        if !map.contains_key(old) {
            bail!("Level '{old}' は存在しません");
        }
        if map.contains_key(new) {
            bail!("Level '{new}' は既に存在します");
        }
        let mut lvl = map.remove(old).expect("checked above");
        lvl.name = new.to_string();
        map.insert(new.to_string(), lvl);
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        let mut map = self.storage.write().expect("RwLock poisoned");
        if map.remove(name).is_none() {
            bail!("Level '{name}' は存在しません");
        }
        Ok(())
    }

    fn import_base_image(&self, _level_name: &str, source: &Path) -> Result<String> {
        // InMemory はファイルを置かないので basename だけ返す
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow!("source path has no file extension"))?
            .to_lowercase();
        Ok(format!("base.{ext}"))
    }

    fn delete_base_image(&self, _level_name: &str, _basename: &str) -> Result<()> {
        // InMemory はファイルを置かないので no-op
        Ok(())
    }
}

pub struct FilesystemLevelRepository {
    levels_dir: PathBuf,
}

impl FilesystemLevelRepository {
    /// `{workspace_dir}/data/levels/` (YAML と base 画像の両方) を読み書きする repository を返す。
    ///
    /// `workspace/assets/` 配下はユーザーが自由に管理する素材保管領域として残し、
    /// editor / engine は触らない方針 (Character の sprite と整合)。
    #[must_use]
    pub fn new(workspace_dir: &Path) -> Self {
        Self {
            levels_dir: workspace_dir.join("data").join(LEVELS_DIR),
        }
    }

    /// テスト等で levels_dir を直接指定する用途。
    #[must_use]
    pub fn from_dir(levels_dir: PathBuf) -> Self {
        Self { levels_dir }
    }

    fn path_for(&self, name: &str) -> PathBuf {
        self.levels_dir.join(format!("{name}.{FILE_EXT}"))
    }

    /// `data/levels/{name}/` を返す。base 画像はこの下に置かれる。
    fn level_dir(&self, name: &str) -> PathBuf {
        self.levels_dir.join(name)
    }
}

impl FilesystemLevelRepository {
    /// `data/levels/{name}/{base}` が PNG なら header を読んで dim を返す。
    /// PNG 以外・不在・破損は None (clamp なしで動作)。
    fn read_base_dimensions(&self, name: &str, base: &str) -> Option<[u32; 2]> {
        let img_path = self.level_dir(name).join(base);
        if !img_path.exists() {
            return None;
        }
        let ext = img_path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase);
        if ext.as_deref() != Some("png") {
            return None;
        }
        crate::shared::read_png_dimensions(&img_path).ok()
    }
}

impl LevelRepository for FilesystemLevelRepository {
    fn load(&self, name: &str) -> Result<Level> {
        let path = self.path_for(name);
        if !path.exists() {
            return Ok(Level::with_defaults(name));
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    "levels/{}.yml の読み込みに失敗: {} (default にフォールバック)",
                    name,
                    e
                );
                return Ok(Level::with_defaults(name));
            }
        };
        match serde_saphyr::from_str::<Level>(&content) {
            Ok(mut lvl) => {
                lvl.name = name.to_string();
                lvl.base_dimensions = self.read_base_dimensions(name, &lvl.base);
                Ok(lvl)
            }
            Err(e) => {
                tracing::warn!(
                    "levels/{}.yml の parse に失敗: {} (default にフォールバック)",
                    name,
                    e
                );
                Ok(Level::with_defaults(name))
            }
        }
    }

    fn get(&self, name: &str) -> Result<Option<Level>> {
        let path = self.path_for(name);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        // parse 失敗時も None ではなく default + name で返す (load と整合)。
        // 「ファイルがあるのに parse できない」状態でも詳細ページを開けるようにするため。
        match serde_saphyr::from_str::<Level>(&content) {
            Ok(mut lvl) => {
                lvl.name = name.to_string();
                lvl.base_dimensions = self.read_base_dimensions(name, &lvl.base);
                Ok(Some(lvl))
            }
            Err(e) => {
                tracing::warn!(
                    "levels/{}.yml の parse に失敗: {} (default にフォールバック)",
                    name,
                    e
                );
                Ok(Some(Level::with_defaults(name)))
            }
        }
    }

    fn list(&self) -> Result<Vec<String>> {
        if !self.levels_dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&self.levels_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some(FILE_EXT) {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    fn create(&self, level: &Level) -> Result<()> {
        if self.path_for(&level.name).exists() {
            bail!("Level '{}' は既に存在します", level.name);
        }
        self.save(level)
    }

    fn save(&self, level: &Level) -> Result<()> {
        fs::create_dir_all(&self.levels_dir)?;
        let yaml = serde_saphyr::to_string(level)?;
        fs::write(self.path_for(&level.name), yaml)?;
        Ok(())
    }

    fn rename(&self, old: &str, new: &str) -> Result<()> {
        if old == new {
            bail!("rename: 旧名と新名が同じです");
        }
        let old_path = self.path_for(old);
        let new_path = self.path_for(new);
        if !old_path.exists() {
            bail!("Level '{old}' は存在しません");
        }
        if new_path.exists() {
            bail!("Level '{new}' は既に存在します");
        }
        fs::rename(&old_path, &new_path)?;
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        let path = self.path_for(name);
        if !path.exists() {
            bail!("Level '{name}' は存在しません");
        }
        fs::remove_file(&path)?;
        Ok(())
    }

    fn import_base_image(&self, level_name: &str, source: &Path) -> Result<String> {
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow!("source path has no file extension"))?
            .to_lowercase();
        let basename = format!("base.{ext}");
        let target_dir = self.level_dir(level_name);
        fs::create_dir_all(&target_dir)?;
        fs::copy(source, target_dir.join(&basename))?;
        Ok(basename)
    }

    fn delete_base_image(&self, level_name: &str, basename: &str) -> Result<()> {
        let target = self.level_dir(level_name).join(basename);
        if target.exists() {
            fs::remove_file(target)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Area, OpponentTrigger};
    use super::*;

    fn sample(name: &str) -> Level {
        Level {
            name: name.to_string(),
            base: "base.png".to_string(),
            base_dimensions: None,
            areas: vec![Area {
                near_z: 60,
                far_z: -60,
                near_min_x: 100,
                near_max_x: 1180,
                far_min_x: -10,
                far_max_x: 1280,
            }],
            camera_start_x: 10,
            camera_start_y: 20,
            player_spawn_x: 50,
            player_spawn_z: -8,
            player_respawn_y: 32,
            opponent_triggers: vec![OpponentTrigger {
                character_name: "thug".to_string(),
                trigger_x: 200,
                spawn_x: 480,
                spawn_y: 0,
                spawn_z: 0,
            }],
            gravity_scale: None,
        }
    }

    #[test]
    fn in_memory_save_and_load() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        assert_eq!(repo.load("ct")?, Level::with_defaults("ct"));

        let lvl = sample("ct");
        repo.save(&lvl)?;
        assert_eq!(repo.load("ct")?, lvl);
        Ok(())
    }

    #[test]
    fn in_memory_list_returns_sorted_names() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        repo.save(&sample("zoo"))?;
        repo.save(&sample("alpha"))?;
        repo.save(&sample("middle"))?;
        assert_eq!(repo.list()?, vec!["alpha", "middle", "zoo"]);
        Ok(())
    }

    #[test]
    fn in_memory_get_returns_none_when_missing() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        assert_eq!(repo.get("ct")?, None);
        repo.save(&sample("ct"))?;
        assert_eq!(repo.get("ct")?, Some(sample("ct")));
        Ok(())
    }

    #[test]
    fn in_memory_create_rejects_duplicates() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        repo.create(&sample("ct"))?;
        assert!(repo.create(&sample("ct")).is_err());
        Ok(())
    }

    #[test]
    fn in_memory_rename_moves_entry() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        repo.save(&sample("ct"))?;
        repo.rename("ct", "training")?;
        assert_eq!(repo.get("ct")?, None);
        let renamed = repo.get("training")?.expect("renamed entry should exist");
        assert_eq!(renamed.name, "training");
        // 中身は保持される (name 以外)
        assert_eq!(renamed.base, sample("ct").base);
        Ok(())
    }

    #[test]
    fn in_memory_rename_rejects_collision() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        repo.save(&sample("ct"))?;
        repo.save(&sample("training"))?;
        assert!(repo.rename("ct", "training").is_err());
        Ok(())
    }

    #[test]
    fn in_memory_rename_rejects_missing_source() {
        let repo = InMemoryLevelRepository::new();
        assert!(repo.rename("ct", "training").is_err());
    }

    #[test]
    fn in_memory_delete_removes_entry() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        repo.save(&sample("ct"))?;
        repo.delete("ct")?;
        assert_eq!(repo.get("ct")?, None);
        assert!(repo.delete("ct").is_err());
        Ok(())
    }

    #[test]
    fn in_memory_exists() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        assert!(!repo.exists("ct")?);
        repo.save(&sample("ct"))?;
        assert!(repo.exists("ct")?);
        Ok(())
    }

    #[test]
    fn filesystem_save_and_load_round_trips() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));

        let lvl = sample("ct");
        repo.save(&lvl)?;
        assert!(repo.path_for("ct").exists(), "save should create the file");
        assert_eq!(repo.load("ct")?, lvl);
        Ok(())
    }

    #[test]
    fn filesystem_load_returns_default_when_file_missing() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().to_path_buf());
        assert_eq!(repo.load("ct")?, Level::with_defaults("ct"));
        Ok(())
    }

    #[test]
    fn filesystem_get_returns_none_when_file_missing() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().to_path_buf());
        assert_eq!(repo.get("ct")?, None);
        Ok(())
    }

    #[test]
    fn filesystem_get_returns_some_when_file_exists() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        repo.save(&sample("ct"))?;
        assert_eq!(repo.get("ct")?, Some(sample("ct")));
        Ok(())
    }

    #[test]
    fn filesystem_load_fills_in_defaults_for_missing_fields() -> Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path())?;
        fs::write(dir.path().join("ct.yml"), "camera_start_x: 200\n")?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().to_path_buf());

        let lvl = repo.load("ct")?;
        assert_eq!(lvl.name, "ct");
        assert_eq!(lvl.camera_start_x, 200);
        let default = Level::with_defaults("ct");
        assert_eq!(lvl.base, default.base);
        assert_eq!(lvl.areas, default.areas);
        assert_eq!(lvl.camera_start_y, default.camera_start_y);
        assert_eq!(lvl.player_spawn_x, default.player_spawn_x);
        assert_eq!(lvl.player_spawn_z, default.player_spawn_z);
        assert_eq!(lvl.player_respawn_y, default.player_respawn_y);
        assert!(lvl.opponent_triggers.is_empty());
        Ok(())
    }

    #[test]
    fn filesystem_load_parses_opponent_triggers() -> Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path())?;
        fs::write(
            dir.path().join("ct.yml"),
            "opponent_triggers:\n  - character_name: thug\n    trigger_x: 200\n    spawn_x: 480\n    spawn_y: 0\n    spawn_z: -16\n  - character_name: boss\n    trigger_x: 480\n    spawn_x: 600\n    spawn_y: 64\n    spawn_z: 0\n",
        )?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().to_path_buf());

        let lvl = repo.load("ct")?;
        assert_eq!(lvl.opponent_triggers.len(), 2);
        let first = &lvl.opponent_triggers[0];
        assert_eq!(first.character_name, "thug");
        assert_eq!(first.trigger_x, 200);
        assert_eq!(first.spawn_x, 480);
        assert_eq!(first.spawn_z, -16);
        let second = &lvl.opponent_triggers[1];
        assert_eq!(second.character_name, "boss");
        assert_eq!(second.spawn_y, 64);
        Ok(())
    }

    #[test]
    fn filesystem_save_omits_empty_opponent_triggers() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        let mut lvl = Level::with_defaults("ct");
        lvl.camera_start_x = 12;
        // opponent_triggers は空のまま
        repo.save(&lvl)?;
        let yaml = fs::read_to_string(repo.path_for("ct"))?;
        assert!(
            !yaml.contains("opponent_triggers"),
            "空の opponent_triggers は YAML から省略されるべき。実際: {yaml}"
        );
        Ok(())
    }

    #[test]
    fn filesystem_load_returns_default_when_yaml_is_broken() -> Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path())?;
        fs::write(
            dir.path().join("ct.yml"),
            "this is :: not a valid yaml :: at all",
        )?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().to_path_buf());
        assert_eq!(repo.load("ct")?, Level::with_defaults("ct"));
        Ok(())
    }

    #[test]
    fn filesystem_list_enumerates_yml_stems() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        repo.save(&sample("ct"))?;
        repo.save(&sample("training"))?;
        fs::write(dir.path().join("data/levels/notes.txt"), "ignore me")?;
        assert_eq!(repo.list()?, vec!["ct", "training"]);
        Ok(())
    }

    #[test]
    fn filesystem_new_uses_workspace_data_layout() {
        let repo = FilesystemLevelRepository::new(&PathBuf::from("/ws"));
        let path = repo.path_for("ct");
        assert!(
            path.ends_with("data/levels/ct.yml") || path.ends_with("data\\levels\\ct.yml"),
            "actual: {}",
            path.display()
        );
    }

    #[test]
    fn filesystem_create_rejects_duplicates() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        repo.create(&sample("ct"))?;
        assert!(repo.create(&sample("ct")).is_err());
        Ok(())
    }

    #[test]
    fn filesystem_rename_moves_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        repo.save(&sample("ct"))?;
        repo.rename("ct", "training")?;
        assert!(!repo.path_for("ct").exists());
        assert!(repo.path_for("training").exists());
        // 中身は保持される (name はファイル名由来なので load 経由で読み戻したときに training になる)
        let loaded = repo.load("training")?;
        assert_eq!(loaded.name, "training");
        assert_eq!(loaded.base, sample("ct").base);
        Ok(())
    }

    #[test]
    fn filesystem_rename_rejects_collision() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        repo.save(&sample("ct"))?;
        repo.save(&sample("training"))?;
        assert!(repo.rename("ct", "training").is_err());
        Ok(())
    }

    #[test]
    fn filesystem_rename_rejects_missing_source() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        assert!(repo.rename("ct", "training").is_err());
        Ok(())
    }

    #[test]
    fn filesystem_delete_removes_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        repo.save(&sample("ct"))?;
        repo.delete("ct")?;
        assert!(!repo.path_for("ct").exists());
        assert!(repo.delete("ct").is_err());
        Ok(())
    }

    #[test]
    fn filesystem_exists() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));
        assert!(!repo.exists("ct")?);
        repo.save(&sample("ct"))?;
        assert!(repo.exists("ct")?);
        Ok(())
    }

    #[test]
    fn in_memory_import_base_image_returns_basename_from_extension() -> Result<()> {
        let repo = InMemoryLevelRepository::new();
        let basename = repo.import_base_image("ct", &PathBuf::from("/tmp/mountain.PNG"))?;
        assert_eq!(basename, "base.png");
        Ok(())
    }

    #[test]
    fn in_memory_import_base_image_rejects_missing_extension() {
        let repo = InMemoryLevelRepository::new();
        assert!(
            repo.import_base_image("ct", &PathBuf::from("/tmp/no_ext"))
                .is_err()
        );
    }

    #[test]
    fn filesystem_import_base_image_copies_and_returns_basename() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("source.PNG");
        fs::write(&source, b"fake png bytes")?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));

        let basename = repo.import_base_image("ct", &source)?;
        assert_eq!(basename, "base.png");
        let copied = dir.path().join("data/levels/ct/base.png");
        assert!(copied.exists(), "copied file should exist at {copied:?}");
        assert_eq!(fs::read(&copied)?, b"fake png bytes");
        Ok(())
    }

    #[test]
    fn filesystem_import_base_image_overwrites_existing() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let source1 = dir.path().join("a.png");
        let source2 = dir.path().join("b.png");
        fs::write(&source1, b"old")?;
        fs::write(&source2, b"new")?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));

        repo.import_base_image("ct", &source1)?;
        repo.import_base_image("ct", &source2)?;
        let copied = dir.path().join("data/levels/ct/base.png");
        assert_eq!(fs::read(&copied)?, b"new");
        Ok(())
    }

    #[test]
    fn filesystem_delete_base_image_removes_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("source.png");
        fs::write(&source, b"x")?;
        let repo = FilesystemLevelRepository::from_dir(dir.path().join("data/levels"));

        let basename = repo.import_base_image("ct", &source)?;
        let copied = dir.path().join("data/levels/ct").join(&basename);
        assert!(copied.exists());

        repo.delete_base_image("ct", &basename)?;
        assert!(!copied.exists());
        // 不在時の再削除は no-op で Ok
        repo.delete_base_image("ct", &basename)?;
        Ok(())
    }
}
