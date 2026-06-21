//! Character / Animation / SpriteGroup の YAML ローダー (FSD: api segment)。
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::model::{Animation, Character, SpriteGroup};
use crate::shared::config::RuntimePaths;

impl Character {
    pub fn load_from_file(path: &Path, name: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("character YAML を読めない: {}", path.display()))?;
        let mut character: Self = serde_saphyr::from_str(&text)
            .with_context(|| format!("character YAML をパースできない: {}", path.display()))?;
        if character.name.is_empty() {
            name.clone_into(&mut character.name);
        }
        Ok(character)
    }

    /// `runtime/data/characters/{name}.yml` を本体としてロードし、
    /// `{name}/sprite-groups/*.yml` と `{name}/animations/*.yml` を walk して
    /// `sprite_groups` / `animations` を populate する。
    ///
    /// サブディレクトリが存在しない場合は空のままにする (warn ログを期待する場面では呼出側で判定)。
    pub fn load_directory(runtime: &RuntimePaths, name: &str) -> Result<Self> {
        let mut character = Self::load_from_file(&runtime.character_file(name), name)?;
        let dir = runtime.character_dir(name);
        character.sprite_groups = load_sprite_groups_in_dir(&dir.join("sprite-groups"))?;
        character.animations = load_animations_in_dir(&dir.join("animations"))?;
        Ok(character)
    }
}

impl Animation {
    pub fn load_from_file(path: &Path, name: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("animation YAML を読めない: {}", path.display()))?;
        let mut anim: Self = serde_saphyr::from_str(&text)
            .with_context(|| format!("animation YAML をパースできない: {}", path.display()))?;
        name.clone_into(&mut anim.name);
        Ok(anim)
    }
}

impl SpriteGroup {
    pub fn load_from_file(path: &Path, name: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("sprite-group YAML を読めない: {}", path.display()))?;
        let mut group: Self = serde_saphyr::from_str(&text)
            .with_context(|| format!("sprite-group YAML をパースできない: {}", path.display()))?;
        name.clone_into(&mut group.name);
        Ok(group)
    }
}

fn load_sprite_groups_in_dir(dir: &Path) -> Result<HashMap<u32, SpriteGroup>> {
    if !dir.is_dir() {
        return Ok(HashMap::new());
    }
    let mut groups = HashMap::new();
    for entry in fs::read_dir(dir)
        .with_context(|| format!("sprite-groups ディレクトリを読めない: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yml") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let group = SpriteGroup::load_from_file(&path, stem)?;
        groups.insert(group.number, group);
    }
    Ok(groups)
}

fn load_animations_in_dir(dir: &Path) -> Result<Vec<Animation>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut anims = Vec::new();
    for entry in fs::read_dir(dir)
        .with_context(|| format!("animations ディレクトリを読めない: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yml") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        anims.push(Animation::load_from_file(&path, stem)?);
    }
    Ok(anims)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::entities::character::{DEFAULT_DEPTH, DEFAULT_HP, Role};

    fn write_yaml(dir: &Path, stem: &str, yaml: &str) -> Result<PathBuf> {
        let path = dir.join(format!("{stem}.yml"));
        fs::write(&path, yaml)?;
        Ok(path)
    }

    fn write_file(path: &PathBuf, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }

    // === Character::load_from_file ===

    #[test]
    fn character_load_round_trip_from_moo_yml_shape() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "name: MooR_01\nthumbnail_path: sprite-groups/thumbnail/sprites/portrait_001.png\nhp: 500\ndepth: 16\n";
        let path = write_yaml(dir.path(), "MooR_01", yaml)?;
        let character = Character::load_from_file(&path, "MooR_01")?;
        assert_eq!(character.name, "MooR_01");
        assert_eq!(
            character.thumbnail_path,
            "sprite-groups/thumbnail/sprites/portrait_001.png",
        );
        assert_eq!(character.hp, 500);
        assert_eq!(character.depth, 16);
        Ok(())
    }

    #[test]
    fn character_missing_name_falls_back_to_provided_arg() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "thumbnail_path: t.png\nhp: 100\ndepth: 16\n";
        let path = write_yaml(dir.path(), "Foo", yaml)?;
        let character = Character::load_from_file(&path, "Foo")?;
        assert_eq!(character.name, "Foo");
        Ok(())
    }

    #[test]
    fn character_defaults_applied_when_fields_missing() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(dir.path(), "Bare", "name: Bare\n")?;
        let character = Character::load_from_file(&path, "Bare")?;
        assert_eq!(character.name, "Bare");
        assert_eq!(character.thumbnail_path, "");
        assert_eq!(character.hp, DEFAULT_HP);
        assert_eq!(character.depth, DEFAULT_DEPTH);
        Ok(())
    }

    #[test]
    fn character_missing_file_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("nope.yml");
        assert!(Character::load_from_file(&path, "nope").is_err());
        Ok(())
    }

    #[test]
    fn character_broken_yaml_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(
            dir.path(),
            "broken",
            "this is :: not a valid yaml :: at all\n",
        )?;
        assert!(Character::load_from_file(&path, "broken").is_err());
        Ok(())
    }

    // === Character::load_directory ===

    #[test]
    fn load_directory_populates_sprite_groups_and_animations() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let runtime = RuntimePaths::from_root(dir.path().to_path_buf());

        write_file(
            &runtime.character_file("MooR_01"),
            "name: MooR_01\nhp: 500\ndepth: 16\n",
        )?;
        write_file(
            &runtime
                .character_dir("MooR_01")
                .join("sprite-groups/walk.yml"),
            "name: walk\nnumber: 1\nsprites:\n- index: 0\n  path: 001.png\n  pivot_point:\n  - 23\n  - 93\n",
        )?;
        write_file(
            &runtime
                .character_dir("MooR_01")
                .join("sprite-groups/idle.yml"),
            "name: idle\nnumber: 0\nsprites:\n- index: 0\n  path: 001.png\n  pivot_point:\n  - 24\n  - 93\n",
        )?;
        write_file(
            &runtime.character_dir("MooR_01").join("animations/walk.yml"),
            "name: walk\nrole: walk\nis_loop: true\nframes:\n- index: 0\n  ticks: 7\n  layers:\n  - index: 0\n    sprite_group_number: 1\n    sprite_index: 0\n",
        )?;
        write_file(
            &runtime.character_dir("MooR_01").join("animations/idle.yml"),
            "name: idle\nrole: idle\nis_loop: true\nframes:\n- index: 0\n  ticks: 3\n  layers:\n  - index: 0\n    sprite_group_number: 0\n    sprite_index: 0\n",
        )?;

        let character = Character::load_directory(&runtime, "MooR_01")?;
        assert_eq!(character.name, "MooR_01");
        assert_eq!(character.hp, 500);
        assert_eq!(character.sprite_groups.len(), 2);
        assert!(character.sprite_groups.contains_key(&0));
        assert!(character.sprite_groups.contains_key(&1));
        assert_eq!(character.sprite_groups[&1].sprites[0].pivot_point, [23, 93]);
        assert_eq!(character.animations.len(), 2);
        let role_names: Vec<&str> = character
            .animations
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        assert!(role_names.contains(&"walk"));
        assert!(role_names.contains(&"idle"));
        Ok(())
    }

    #[test]
    fn load_directory_with_missing_subdirs_returns_empty_collections() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let runtime = RuntimePaths::from_root(dir.path().to_path_buf());
        write_file(&runtime.character_file("Bare"), "name: Bare\n")?;
        let character = Character::load_directory(&runtime, "Bare")?;
        assert!(character.sprite_groups.is_empty());
        assert!(character.animations.is_empty());
        Ok(())
    }

    #[test]
    fn load_directory_ignores_non_yml_files() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let runtime = RuntimePaths::from_root(dir.path().to_path_buf());
        write_file(&runtime.character_file("MooR_01"), "name: MooR_01\n")?;
        write_file(
            &runtime
                .character_dir("MooR_01")
                .join("sprite-groups/walk.yml"),
            "name: walk\nnumber: 1\nsprites: []\n",
        )?;
        write_file(
            &runtime
                .character_dir("MooR_01")
                .join("sprite-groups/README.md"),
            "ignore me\n",
        )?;
        let character = Character::load_directory(&runtime, "MooR_01")?;
        assert_eq!(character.sprite_groups.len(), 1);
        Ok(())
    }

    // === Animation::load_from_file ===

    #[test]
    fn animation_load_round_trip_from_walk_yml_shape() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "name: walk\nrole: walk\nvariant: 0\nis_loop: true\nloop_start_index: 0\nframes:\n- index: 0\n  ticks: 7\n  flip: null\n  pivot_point_offset: null\n  body_box_overrides: null\n  attack_box_overrides: null\n  layers:\n  - index: 0\n    sprite_group_number: 1\n    sprite_index: 0\n    transparency: 1.0\n    flip: null\n    pivot_point_offset: null\n- index: 1\n  ticks: 7\n  flip: null\n  pivot_point_offset: null\n  body_box_overrides: null\n  attack_box_overrides: null\n  layers:\n  - index: 0\n    sprite_group_number: 1\n    sprite_index: 1\n    transparency: 1.0\n    flip: null\n    pivot_point_offset: null\n";
        let path = write_yaml(dir.path(), "walk", yaml)?;
        let anim = Animation::load_from_file(&path, "walk")?;
        assert_eq!(anim.name, "walk");
        assert_eq!(anim.role, Role::Walk);
        assert_eq!(anim.variant, 0);
        assert!(anim.is_loop);
        assert_eq!(anim.loop_start_index, 0);
        assert_eq!(anim.frames.len(), 2);
        assert_eq!(anim.frames[0].ticks, 7);
        assert_eq!(anim.frames[0].layers.len(), 1);
        assert_eq!(anim.frames[0].layers[0].sprite_group_number, 1);
        assert_eq!(anim.frames[0].layers[0].sprite_index, 0);
        assert!((anim.frames[0].layers[0].transparency - 1.0).abs() < f32::EPSILON);
        assert_eq!(anim.frames[1].layers[0].sprite_index, 1);
        Ok(())
    }

    #[test]
    fn animation_role_idle_round_trips() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "role: idle\nis_loop: true\nframes:\n- index: 0\n  ticks: 3\n  layers: []\n";
        let path = write_yaml(dir.path(), "idle", yaml)?;
        let anim = Animation::load_from_file(&path, "idle")?;
        assert_eq!(anim.role, Role::Idle);
        assert!(anim.is_loop);
        Ok(())
    }

    #[test]
    fn animation_name_is_always_taken_from_provided_arg() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "role: walk\nframes: []\n";
        let path = write_yaml(dir.path(), "stem_name", yaml)?;
        let anim = Animation::load_from_file(&path, "stem_name")?;
        assert_eq!(anim.name, "stem_name");
        Ok(())
    }

    #[test]
    fn animation_empty_yaml_uses_defaults() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(dir.path(), "empty", "{}\n")?;
        let anim = Animation::load_from_file(&path, "empty")?;
        assert_eq!(anim.name, "empty");
        assert_eq!(anim.role, Role::Custom);
        assert!(!anim.is_loop);
        assert!(anim.frames.is_empty());
        Ok(())
    }

    #[test]
    fn animation_missing_file_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("nope.yml");
        assert!(Animation::load_from_file(&path, "nope").is_err());
        Ok(())
    }

    #[test]
    fn animation_broken_yaml_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(
            dir.path(),
            "broken",
            "this is :: not a valid yaml :: at all\n",
        )?;
        assert!(Animation::load_from_file(&path, "broken").is_err());
        Ok(())
    }

    // === SpriteGroup::load_from_file ===

    #[test]
    fn sprite_group_load_round_trip_from_walk_yml_shape() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "name: walk\nnumber: 1\nsprites:\n- index: 0\n  path: 001.png\n  pivot_point:\n  - 23\n  - 93\n  body_boxes:\n  - top_left:\n    - 12\n    - 11\n    bottom_right:\n    - 31\n    - 77\n  attack_boxes: null\n- index: 1\n  path: 002.png\n  pivot_point:\n  - 24\n  - 93\n  body_boxes: null\n  attack_boxes: null\n";
        let path = write_yaml(dir.path(), "walk", yaml)?;
        let group = SpriteGroup::load_from_file(&path, "walk")?;
        assert_eq!(group.name, "walk");
        assert_eq!(group.number, 1);
        assert_eq!(group.sprites.len(), 2);
        assert_eq!(group.sprites[0].index, 0);
        assert_eq!(group.sprites[0].path, "001.png");
        assert_eq!(group.sprites[0].pivot_point, [23, 93]);
        assert_eq!(group.sprites[1].pivot_point, [24, 93]);
        Ok(())
    }

    #[test]
    fn sprite_group_empty_yaml_uses_defaults() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(dir.path(), "empty", "{}\n")?;
        let group = SpriteGroup::load_from_file(&path, "empty")?;
        assert_eq!(group.name, "empty");
        assert_eq!(group.number, 0);
        assert!(group.sprites.is_empty());
        Ok(())
    }

    #[test]
    fn sprite_group_missing_file_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("nope.yml");
        assert!(SpriteGroup::load_from_file(&path, "nope").is_err());
        Ok(())
    }

    #[test]
    fn sprite_group_broken_yaml_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(
            dir.path(),
            "broken",
            "this is :: not a valid yaml :: at all\n",
        )?;
        assert!(SpriteGroup::load_from_file(&path, "broken").is_err());
        Ok(())
    }
}
