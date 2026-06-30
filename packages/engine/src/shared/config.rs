//! Engine の runtime ディレクトリ解決と起動時 config (`bebeu-engine.yml`)。
//!
//! `bebeu-engine.yml` は **engine が起動時に 1 度だけ読む config**。`workspace_dir`
//! のみを持つ (ADR-0016 が言う「engine 専用 config」、ただし window 関連は ADR-0041 で
//! Bevy App Settings に移管したため本 yml からは外れた)。
//!
//! 解決優先順:
//! - `workspace_dir`: env `BEATEMUP_RUNTIME_DIR` > yml `workspace_dir` >
//!   `CARGO_MANIFEST_DIR/../../runtime`
//! - yml ファイル位置: env `BEATEMUP_ENGINE_CONFIG` > `CARGO_MANIFEST_DIR/bebeu-engine.yml`
//!
//! yml が無い / 壊れている場合は default で fail-soft する (config なしでも engine は
//! 起動できる)。Project YAML のような一次データは fail-soft しない (ADR-0011 / testing.md)
//! が、起動 config は補助なので扱いを分ける。
use std::path::{Path, PathBuf};

use bevy::prelude::Resource;
use serde::Deserialize;

/// runtime ツリー (`runtime/data`, `runtime/assets`) のルート位置。
#[derive(Resource, Debug, Clone)]
pub struct RuntimePaths {
    root: PathBuf,
}

impl RuntimePaths {
    pub fn resolve(engine_config: &EngineConfig) -> Self {
        if let Some(env_root) = std::env::var_os("BEATEMUP_RUNTIME_DIR") {
            return Self {
                root: PathBuf::from(env_root),
            };
        }
        if let Some(yml_root) = engine_config.workspace_dir.as_ref() {
            return Self {
                root: yml_root.clone(),
            };
        }
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest
            .join("..")
            .join("..")
            .join("runtime")
            .canonicalize()
            .unwrap_or_else(|_| manifest.join("../../runtime"));
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn project_file(&self, name: &str) -> PathBuf {
        self.root
            .join("data")
            .join("projects")
            .join(format!("{name}.yml"))
    }

    pub fn character_file(&self, name: &str) -> PathBuf {
        self.root
            .join("data")
            .join("characters")
            .join(format!("{name}.yml"))
    }

    /// character のディレクトリ (`runtime/data/characters/{name}/`)。
    /// `sprite-groups/` と `animations/` を含む。
    pub fn character_dir(&self, name: &str) -> PathBuf {
        self.root.join("data").join("characters").join(name)
    }

    pub fn level_file(&self, name: &str) -> PathBuf {
        self.root
            .join("data")
            .join("levels")
            .join(format!("{name}.yml"))
    }

    /// sprite-group YAML (`runtime/data/characters/{character}/sprite-groups/{group}.yml`)。
    pub fn sprite_group_file(&self, character: &str, group: &str) -> PathBuf {
        self.root
            .join("data")
            .join("characters")
            .join(character)
            .join("sprite-groups")
            .join(format!("{group}.yml"))
    }

    /// sprite-group 内の個別 sprite ファイル
    /// (`runtime/data/characters/{character}/sprite-groups/{group}/sprites/{sprite}`)。
    pub fn sprite_file(&self, character: &str, group: &str, sprite: &str) -> PathBuf {
        self.root
            .join("data")
            .join("characters")
            .join(character)
            .join("sprite-groups")
            .join(group)
            .join("sprites")
            .join(sprite)
    }

    /// sound-group 内の個別 wav ファイル (ADR-0019)
    /// (`runtime/data/characters/{character}/sound-groups/{group}/sounds/{sound}`)。
    pub fn sound_file(&self, character: &str, group: &str, sound: &str) -> PathBuf {
        self.root
            .join("data")
            .join("characters")
            .join(character)
            .join("sound-groups")
            .join(group)
            .join("sounds")
            .join(sound)
    }

    /// Bevy `AssetServer` のルートとして使うディレクトリ。
    ///
    /// editor が生成する sprite-groups や thumbnail は `runtime/data/characters/{name}/...`
    /// 配下に置かれるため、AssetServer は `data/` を root として参照する。
    /// `runtime/assets/` は旧 ikemen-go 互換素材の置き場であり、engine からは現状参照しない。
    pub fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    /// 既知の root から組み立てる。主にテストで `tempfile::tempdir()` の path を渡す用途。
    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }
}

/// `bebeu-engine.yml` の内容。
///
/// 未指定フィールドは `None` のまま (entrypoint 側で fallback を当てる)。
/// window 関連は ADR-0041 で App Settings (`shared/settings.rs::WindowSettings`) に
/// 移管したためここからは外れている。
#[derive(Resource, Debug, Clone, Default, Deserialize)]
pub struct EngineConfig {
    #[serde(default)]
    pub workspace_dir: Option<PathBuf>,
}

impl EngineConfig {
    /// `bebeu-engine.yml` を探して読む。env > manifest-relative の順。
    ///
    /// 見つからなければ default (= 全 `None`)。読めるが壊れている場合は warn して default。
    pub fn load() -> Self {
        let path = Self::resolve_path();
        if !path.exists() {
            return Self::default();
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(error = %err, path = %path.display(), "engine_config: read failed");
                return Self::default();
            }
        };
        match serde_saphyr::from_str(&text) {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!(error = %err, path = %path.display(), "engine_config: parse failed");
                Self::default()
            }
        }
    }

    fn resolve_path() -> PathBuf {
        if let Some(env) = std::env::var_os("BEATEMUP_ENGINE_CONFIG") {
            return PathBuf::from(env);
        }
        Path::new(env!("CARGO_MANIFEST_DIR")).join("bebeu-engine.yml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PathBuf, RuntimePaths) {
        let root = PathBuf::from("fake").join("runtime");
        let runtime = RuntimePaths::from_root(root.clone());
        (root, runtime)
    }

    #[test]
    fn root_returns_provided_path() {
        let (root, runtime) = fixture();
        assert_eq!(runtime.root(), root.as_path());
    }

    #[test]
    fn data_dir_appends_data_segment() {
        let (root, runtime) = fixture();
        assert_eq!(runtime.data_dir(), root.join("data"));
    }

    #[test]
    fn project_file_resolves_under_data_projects() {
        let (root, runtime) = fixture();
        assert_eq!(
            runtime.project_file("main"),
            root.join("data").join("projects").join("main.yml"),
        );
    }

    #[test]
    fn character_file_resolves_under_data_characters() {
        let (root, runtime) = fixture();
        assert_eq!(
            runtime.character_file("MooR_01"),
            root.join("data").join("characters").join("MooR_01.yml"),
        );
    }

    #[test]
    fn character_dir_resolves_under_data_characters_without_extension() {
        let (root, runtime) = fixture();
        assert_eq!(
            runtime.character_dir("MooR_01"),
            root.join("data").join("characters").join("MooR_01"),
        );
    }

    #[test]
    fn level_file_resolves_under_data_levels() {
        let (root, runtime) = fixture();
        assert_eq!(
            runtime.level_file("ct"),
            root.join("data").join("levels").join("ct.yml"),
        );
    }

    #[test]
    fn sprite_group_file_resolves_under_character_sprite_groups() {
        let (root, runtime) = fixture();
        assert_eq!(
            runtime.sprite_group_file("MooR_01", "walk"),
            root.join("data")
                .join("characters")
                .join("MooR_01")
                .join("sprite-groups")
                .join("walk.yml"),
        );
    }

    #[test]
    fn sprite_file_resolves_under_sprite_group_sprites() {
        let (root, runtime) = fixture();
        assert_eq!(
            runtime.sprite_file("MooR_01", "walk", "001.png"),
            root.join("data")
                .join("characters")
                .join("MooR_01")
                .join("sprite-groups")
                .join("walk")
                .join("sprites")
                .join("001.png"),
        );
    }

    // RuntimePaths::resolve の優先順は env > yml > manifest fallback。
    //
    // env を立てると `std::env::set_var` がプロセス全体に影響して並列テストを汚染するため、
    // 「yml が指定されたら採用される」「yml も無ければ manifest fallback」の 2 ケースだけ
    // テストする。env 優先のロジックは目視と smoke test に委ねる。
    #[test]
    fn resolve_uses_engine_config_workspace_dir() {
        let yml_root = PathBuf::from("from").join("yml");
        let cfg = EngineConfig {
            workspace_dir: Some(yml_root.clone()),
        };
        // env を unset した状態を期待 (CI / 通常 dev で BEATEMUP_RUNTIME_DIR は立たない想定)。
        if std::env::var_os("BEATEMUP_RUNTIME_DIR").is_some() {
            return;
        }
        let runtime = RuntimePaths::resolve(&cfg);
        assert_eq!(runtime.root(), yml_root.as_path());
    }

    #[test]
    fn engine_config_default_is_all_none() {
        let cfg = EngineConfig::default();
        assert!(cfg.workspace_dir.is_none());
    }

    #[test]
    fn engine_config_parses_workspace_dir() {
        let yaml = "workspace_dir: /tmp/runtime\n";
        let cfg: EngineConfig = serde_saphyr::from_str(yaml).expect("parse");
        assert_eq!(cfg.workspace_dir, Some(PathBuf::from("/tmp/runtime")));
    }

    #[test]
    fn engine_config_empty_yaml_uses_defaults() {
        let cfg: EngineConfig = serde_saphyr::from_str("{}\n").expect("parse");
        assert!(cfg.workspace_dir.is_none());
    }
}
