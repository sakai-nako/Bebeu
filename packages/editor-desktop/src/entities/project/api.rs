use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::Project;

const PROJECTS_DIR: &str = "projects";
const FILE_EXT: &str = "yml";

pub trait ProjectRepository: Send + Sync {
    /// 既存 Project の名前一覧を返す (ソート済み)。
    fn list(&self) -> Result<Vec<String>>;

    /// 名前指定で Project をロードする。不在なら error。
    fn get(&self, name: &str) -> Result<Project>;

    /// 新規 Project を作成。同名の Project が既に存在する場合は error。
    fn create(&self, project: &Project) -> Result<()>;

    /// 既存 Project を更新。不在なら error。
    fn update(&self, project: &Project) -> Result<()>;

    /// Project をリネーム。new_name が既存 / old_name が不在なら error。
    fn rename(&self, old_name: &str, new_name: &str) -> Result<()>;

    /// Project を削除。不在なら error。
    fn delete(&self, name: &str) -> Result<()>;
}

pub struct InMemoryProjectRepository {
    storage: RwLock<HashMap<String, Project>>,
}

impl InMemoryProjectRepository {
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryProjectRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectRepository for InMemoryProjectRepository {
    fn list(&self) -> Result<Vec<String>> {
        let map = self.storage.read().expect("RwLock poisoned");
        let mut names: Vec<String> = map.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    fn get(&self, name: &str) -> Result<Project> {
        let map = self.storage.read().expect("RwLock poisoned");
        map.get(name)
            .cloned()
            .ok_or_else(|| anyhow!("Project '{name}' は存在しません"))
    }

    fn create(&self, project: &Project) -> Result<()> {
        let mut map = self.storage.write().expect("RwLock poisoned");
        if map.contains_key(&project.name) {
            let name = &project.name;
            return Err(anyhow!("Project '{name}' は既に存在します"));
        }
        map.insert(project.name.clone(), project.clone());
        Ok(())
    }

    fn update(&self, project: &Project) -> Result<()> {
        let mut map = self.storage.write().expect("RwLock poisoned");
        if !map.contains_key(&project.name) {
            let name = &project.name;
            return Err(anyhow!("Project '{name}' は存在しません"));
        }
        map.insert(project.name.clone(), project.clone());
        Ok(())
    }

    fn rename(&self, old_name: &str, new_name: &str) -> Result<()> {
        let mut map = self.storage.write().expect("RwLock poisoned");
        if !map.contains_key(old_name) {
            return Err(anyhow!("Project '{old_name}' は存在しません"));
        }
        if map.contains_key(new_name) {
            return Err(anyhow!("Project '{new_name}' は既に存在します"));
        }
        let mut proj = map.remove(old_name).expect("contains_key で確認済み");
        proj.name = new_name.to_string();
        map.insert(new_name.to_string(), proj);
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        let mut map = self.storage.write().expect("RwLock poisoned");
        map.remove(name)
            .map(|_| ())
            .ok_or_else(|| anyhow!("Project '{name}' は存在しません"))
    }
}

pub struct FilesystemProjectRepository {
    projects_dir: PathBuf,
}

impl FilesystemProjectRepository {
    /// `{workspace_dir}/data/projects/` を読み書きする repository を返す。
    #[must_use]
    pub fn new(workspace_dir: &Path) -> Self {
        Self {
            projects_dir: workspace_dir.join("data").join(PROJECTS_DIR),
        }
    }

    #[must_use]
    pub fn from_dir(projects_dir: PathBuf) -> Self {
        Self { projects_dir }
    }

    #[must_use]
    pub fn projects_dir(&self) -> &PathBuf {
        &self.projects_dir
    }

    fn path_for(&self, name: &str) -> PathBuf {
        self.projects_dir.join(format!("{name}.{FILE_EXT}"))
    }
}

impl ProjectRepository for FilesystemProjectRepository {
    fn list(&self) -> Result<Vec<String>> {
        if !self.projects_dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&self.projects_dir)? {
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

    fn get(&self, name: &str) -> Result<Project> {
        let path = self.path_for(name);
        if !path.exists() {
            return Err(anyhow!("Project '{name}' は存在しません"));
        }
        let content = fs::read_to_string(&path)?;
        let mut proj = serde_saphyr::from_str::<Project>(&content)
            .map_err(|e| anyhow!("Project '{name}' の parse に失敗: {e}"))?;
        proj.name = name.to_string();
        Ok(proj)
    }

    fn create(&self, project: &Project) -> Result<()> {
        let path = self.path_for(&project.name);
        if path.exists() {
            let name = &project.name;
            return Err(anyhow!("Project '{name}' は既に存在します"));
        }
        fs::create_dir_all(&self.projects_dir)?;
        let yaml = serde_saphyr::to_string(project)?;
        fs::write(&path, yaml)?;
        Ok(())
    }

    fn update(&self, project: &Project) -> Result<()> {
        let path = self.path_for(&project.name);
        if !path.exists() {
            let name = &project.name;
            return Err(anyhow!("Project '{name}' は存在しません"));
        }
        let yaml = serde_saphyr::to_string(project)?;
        fs::write(&path, yaml)?;
        Ok(())
    }

    fn rename(&self, old_name: &str, new_name: &str) -> Result<()> {
        let old_path = self.path_for(old_name);
        let new_path = self.path_for(new_name);
        if !old_path.exists() {
            return Err(anyhow!("Project '{old_name}' は存在しません"));
        }
        if new_path.exists() {
            return Err(anyhow!("Project '{new_name}' は既に存在します"));
        }
        fs::rename(&old_path, &new_path)?;
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        let path = self.path_for(name);
        if !path.exists() {
            return Err(anyhow!("Project '{name}' は存在しません"));
        }
        fs::remove_file(&path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::Resolution;
    use super::*;

    fn sample(name: &str) -> Project {
        Project {
            name: name.to_string(),
            resolution: Resolution {
                width: 1280,
                height: 720,
            },
            players: vec!["MooR_01".to_string()],
            opponents: vec!["MooR_02".to_string()],
            levels: vec!["ct".to_string()],
            ..Project::default()
        }
    }

    #[test]
    fn in_memory_create_and_get() -> Result<()> {
        let repo = InMemoryProjectRepository::new();
        repo.create(&sample("default"))?;
        assert_eq!(repo.get("default")?, sample("default"));
        Ok(())
    }

    #[test]
    fn in_memory_create_rejects_duplicate() {
        let repo = InMemoryProjectRepository::new();
        repo.create(&sample("a"))
            .expect("first create should succeed");
        assert!(repo.create(&sample("a")).is_err());
    }

    #[test]
    fn in_memory_update_requires_existing() {
        let repo = InMemoryProjectRepository::new();
        assert!(repo.update(&sample("a")).is_err());
    }

    #[test]
    fn in_memory_get_missing_is_error() {
        let repo = InMemoryProjectRepository::new();
        assert!(repo.get("missing").is_err());
    }

    #[test]
    fn in_memory_rename_moves_entry_and_updates_name() -> Result<()> {
        let repo = InMemoryProjectRepository::new();
        repo.create(&sample("old"))?;
        repo.rename("old", "new")?;
        assert!(repo.get("old").is_err());
        assert_eq!(repo.get("new")?.name, "new");
        Ok(())
    }

    #[test]
    fn in_memory_rename_rejects_duplicate_target() -> Result<()> {
        let repo = InMemoryProjectRepository::new();
        repo.create(&sample("a"))?;
        repo.create(&sample("b"))?;
        assert!(repo.rename("a", "b").is_err());
        Ok(())
    }

    #[test]
    fn in_memory_delete_removes_entry() -> Result<()> {
        let repo = InMemoryProjectRepository::new();
        repo.create(&sample("a"))?;
        repo.delete("a")?;
        assert!(repo.get("a").is_err());
        Ok(())
    }

    #[test]
    fn in_memory_list_returns_sorted_names() -> Result<()> {
        let repo = InMemoryProjectRepository::new();
        repo.create(&sample("zoo"))?;
        repo.create(&sample("alpha"))?;
        repo.create(&sample("middle"))?;
        assert_eq!(repo.list()?, vec!["alpha", "middle", "zoo"]);
        Ok(())
    }

    #[test]
    fn filesystem_create_and_get_round_trips() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().join("projects"));
        repo.create(&sample("default"))?;
        assert!(repo.path_for("default").exists());
        assert_eq!(repo.get("default")?, sample("default"));
        Ok(())
    }

    #[test]
    fn filesystem_get_returns_error_when_missing() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().to_path_buf());
        assert!(repo.get("missing").is_err());
        Ok(())
    }

    #[test]
    fn filesystem_create_rejects_duplicate() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().to_path_buf());
        repo.create(&sample("a"))?;
        assert!(repo.create(&sample("a")).is_err());
        Ok(())
    }

    #[test]
    fn filesystem_update_requires_existing() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().to_path_buf());
        assert!(repo.update(&sample("a")).is_err());
        Ok(())
    }

    #[test]
    fn filesystem_rename_round_trip() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().to_path_buf());
        repo.create(&sample("old"))?;
        repo.rename("old", "new")?;
        assert!(!repo.path_for("old").exists());
        assert!(repo.path_for("new").exists());
        assert_eq!(repo.get("new")?.name, "new");
        Ok(())
    }

    #[test]
    fn filesystem_rename_rejects_duplicate_target() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().to_path_buf());
        repo.create(&sample("a"))?;
        repo.create(&sample("b"))?;
        assert!(repo.rename("a", "b").is_err());
        Ok(())
    }

    #[test]
    fn filesystem_delete_removes_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().to_path_buf());
        repo.create(&sample("a"))?;
        repo.delete("a")?;
        assert!(!repo.path_for("a").exists());
        Ok(())
    }

    #[test]
    fn filesystem_list_enumerates_yml_stems() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().join("projects"));
        repo.create(&sample("alpha"))?;
        repo.create(&sample("beta"))?;
        // 関係ないファイルは無視されること
        fs::write(dir.path().join("projects").join("notes.txt"), "ignore me")?;
        assert_eq!(repo.list()?, vec!["alpha", "beta"]);
        Ok(())
    }

    #[test]
    fn filesystem_load_fills_in_defaults_for_missing_fields() -> Result<()> {
        // 古い / 部分的な project.yml でも `#[serde(default)]` で空配列が補われること
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path())?;
        fs::write(
            dir.path().join("a.yml"),
            "resolution:\n  width: 480\n  height: 270\n",
        )?;
        let repo = FilesystemProjectRepository::from_dir(dir.path().to_path_buf());
        let p = repo.get("a")?;
        assert_eq!(p.name, "a");
        assert_eq!(p.resolution.width, 480);
        assert_eq!(p.resolution.height, 270);
        assert!(p.players.is_empty());
        assert!(p.opponents.is_empty());
        assert!(p.levels.is_empty());
        Ok(())
    }

    #[test]
    fn filesystem_new_uses_workspace_data_layout() {
        let repo = FilesystemProjectRepository::new(&PathBuf::from("/ws"));
        let path = repo.path_for("default");
        assert!(
            path.ends_with("data/projects/default.yml")
                || path.ends_with("data\\projects\\default.yml"),
            "actual: {}",
            path.display()
        );
    }
}
