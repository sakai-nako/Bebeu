//! Level の YAML ローダー (FSD: api segment)。
use std::path::Path;

use anyhow::{Context, Result};

use super::model::Level;

impl Level {
    pub fn load_from_file(path: &Path, name: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("level YAML を読めない: {}", path.display()))?;
        let mut level: Self = serde_saphyr::from_str(&text)
            .with_context(|| format!("level YAML をパースできない: {}", path.display()))?;
        name.clone_into(&mut level.name);
        Ok(level)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::entities::level::Area;

    fn write_yaml(dir: &Path, stem: &str, yaml: &str) -> Result<PathBuf> {
        let path = dir.join(format!("{stem}.yml"));
        fs::write(&path, yaml)?;
        Ok(path)
    }

    #[test]
    fn load_round_trip_from_ct_yml_shape() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "base: base.png\nareas:\n  - near_z: 214\n    far_z: 163\n    near_min_x: 0\n    near_max_x: 906\n    far_min_x: 0\n    far_max_x: 906\ncamera_start_x: 0\ncamera_start_y: 8\nplayer_spawn_x: 28\nplayer_spawn_z: 180\nplayer_respawn_y: 0\nopponent_triggers:\n  - character_name: MooR_01\n    trigger_x: 134\n    spawn_x: 238\n    spawn_y: 30\n    spawn_z: 182\n";
        let path = write_yaml(dir.path(), "ct", yaml)?;
        let level = Level::load_from_file(&path, "ct")?;
        assert_eq!(level.name, "ct");
        assert_eq!(level.base, "base.png");
        assert_eq!(level.areas.len(), 1);
        assert_eq!(level.areas[0].near_z, 214);
        assert_eq!(level.areas[0].far_z, 163);
        assert!(level.areas[0].near_z >= level.areas[0].far_z);
        assert_eq!(level.camera_start_x, 0);
        assert_eq!(level.camera_start_y, 8);
        assert_eq!(level.player_spawn_x, 28);
        assert_eq!(level.player_spawn_z, 180);
        assert_eq!(level.player_respawn_y, 0);
        assert_eq!(level.opponent_triggers.len(), 1);
        assert_eq!(level.opponent_triggers[0].character_name, "MooR_01");
        assert_eq!(level.opponent_triggers[0].trigger_x, 134);
        assert_eq!(level.opponent_triggers[0].spawn_x, 238);
        assert_eq!(level.opponent_triggers[0].spawn_y, 30);
        assert_eq!(level.opponent_triggers[0].spawn_z, 182);
        Ok(())
    }

    #[test]
    fn empty_yaml_uses_defaults() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(dir.path(), "empty", "{}\n")?;
        let level = Level::load_from_file(&path, "empty")?;
        assert_eq!(level.name, "empty");
        assert_eq!(level.base, "base.png");
        assert_eq!(level.areas, vec![Area::default()]);
        assert_eq!(level.camera_start_x, 0);
        assert!(level.opponent_triggers.is_empty());
        assert!(level.gravity_scale.is_none());
        Ok(())
    }

    #[test]
    fn name_is_always_taken_from_provided_arg() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "base: stage.png\n";
        let path = write_yaml(dir.path(), "stem_name", yaml)?;
        let level = Level::load_from_file(&path, "stem_name")?;
        assert_eq!(level.name, "stem_name");
        Ok(())
    }

    #[test]
    fn missing_file_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("nope.yml");
        assert!(Level::load_from_file(&path, "nope").is_err());
        Ok(())
    }

    #[test]
    fn broken_yaml_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(dir.path(), "broken", "this is :: not a valid yaml :: at all\n")?;
        assert!(Level::load_from_file(&path, "broken").is_err());
        Ok(())
    }
}
