//! Engine の runtime ディレクトリ解決 (旧 `internal/engine/shared/config`)。
//!
//! 開発時は `CARGO_MANIFEST_DIR/../../runtime` を絶対パスとして解決する。
//! `BEATEMUP_RUNTIME_DIR` が設定されていればそちらを優先 (配布バイナリや CI で上書きする用途)。
use std::path::{Path, PathBuf};

use bevy::prelude::Resource;

/// runtime ツリー (`runtime/data`, `runtime/assets`) のルート位置。
#[derive(Resource, Debug, Clone)]
pub struct RuntimePaths {
    root: PathBuf,
}

impl RuntimePaths {
    pub fn resolve() -> Self {
        if let Some(env_root) = std::env::var_os("BEATEMUP_RUNTIME_DIR") {
            return Self { root: PathBuf::from(env_root) };
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
        self.root.join("data").join("projects").join(format!("{name}.yml"))
    }

    pub fn character_file(&self, name: &str) -> PathBuf {
        self.root.join("data").join("characters").join(format!("{name}.yml"))
    }

    /// character のディレクトリ (`runtime/data/characters/{name}/`)。
    /// `sprite-groups/` と `animations/` を含む。
    pub fn character_dir(&self, name: &str) -> PathBuf {
        self.root.join("data").join("characters").join(name)
    }

    pub fn level_file(&self, name: &str) -> PathBuf {
        self.root.join("data").join("levels").join(format!("{name}.yml"))
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
}
