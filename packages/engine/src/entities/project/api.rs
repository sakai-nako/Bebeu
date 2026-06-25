//! Project の YAML ローダー (FSD: api segment)。
use std::path::Path;

use anyhow::{Context, Result};

use super::model::Project;

impl Project {
    pub fn load_from_file(path: &Path, name: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("project YAML を読めない: {}", path.display()))?;
        let mut project: Self = serde_saphyr::from_str(&text)
            .with_context(|| format!("project YAML をパースできない: {}", path.display()))?;
        name.clone_into(&mut project.name);
        Ok(project)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::entities::project::Resolution;

    fn write_yaml(dir: &Path, stem: &str, yaml: &str) -> Result<PathBuf> {
        let path = dir.join(format!("{stem}.yml"));
        fs::write(&path, yaml)?;
        Ok(path)
    }

    #[test]
    fn load_round_trip_from_main_yml_shape() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let yaml = "resolution:\n  width: 384\n  height: 216\nplayers:\n  - MooR_01\nopponents:\n  - MooR_01\nlevels:\n  - ct\n";
        let path = write_yaml(dir.path(), "main", yaml)?;
        let project = Project::load_from_file(&path, "main")?;
        assert_eq!(project.name, "main");
        assert_eq!(
            project.resolution,
            Resolution {
                width: 384,
                height: 216
            }
        );
        assert_eq!(project.players, vec!["MooR_01"]);
        assert_eq!(project.opponents, vec!["MooR_01"]);
        assert_eq!(project.levels, vec!["ct"]);
        Ok(())
    }

    #[test]
    fn name_is_always_taken_from_provided_arg() -> Result<()> {
        // Project.name は #[serde(skip)] のため YAML から復元せず、呼び出し側の値で埋める。
        let dir = tempfile::tempdir()?;
        let yaml = "players: []\nopponents: []\nlevels: []\n";
        let path = write_yaml(dir.path(), "from_arg", yaml)?;
        let project = Project::load_from_file(&path, "from_arg")?;
        assert_eq!(project.name, "from_arg");
        Ok(())
    }

    #[test]
    fn empty_yaml_uses_defaults() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = write_yaml(dir.path(), "empty", "{}\n")?;
        let project = Project::load_from_file(&path, "empty")?;
        assert_eq!(project.name, "empty");
        assert_eq!(project.resolution, Resolution::default());
        assert!(project.players.is_empty());
        assert!(project.opponents.is_empty());
        assert!(project.levels.is_empty());
        Ok(())
    }

    #[test]
    fn missing_file_is_error() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("nope.yml");
        assert!(Project::load_from_file(&path, "nope").is_err());
        Ok(())
    }

    #[test]
    fn broken_yaml_is_error() -> Result<()> {
        // Project は一次データなので fail-soft しない (testing.md L81)。
        let dir = tempfile::tempdir()?;
        let path = write_yaml(
            dir.path(),
            "broken",
            "this is :: not a valid yaml :: at all\n",
        )?;
        assert!(Project::load_from_file(&path, "broken").is_err());
        Ok(())
    }

    #[test]
    fn sample_minimal_project_yaml_parses_with_all_hud_kinds() -> Result<()> {
        // sample-projects/minimal の main.yml は HUD 3 種 (player_hp_bar / enemy_hp_bar /
        // enemy_overhead_hp_bar) を含み、スキーマ変更で壊れたらここで弾く。
        use crate::entities::project::{EnemyTarget, HudElement};
        use crate::shared::PlayerId;

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../sample-projects/minimal/data/projects/main.yml");
        let project = Project::load_from_file(&path, "main")?;
        assert_eq!(project.hud.elements.len(), 3);

        let HudElement::PlayerHpBar(p1) = &project.hud.elements[0] else {
            panic!("expected player_hp_bar at index 0");
        };
        assert_eq!(p1.id.as_deref(), Some("p1_hp"));
        assert_eq!(p1.target, PlayerId::P1);

        let HudElement::EnemyHpBar(engaged) = &project.hud.elements[1] else {
            panic!("expected enemy_hp_bar at index 1");
        };
        assert_eq!(engaged.target, EnemyTarget::LastEngagedBy(PlayerId::P1));
        let at = engaged.anchor_to.as_ref().expect("anchor_to set");
        assert_eq!(at.id, "p1_hp");

        let HudElement::EnemyOverheadHpBar(_) = &project.hud.elements[2] else {
            panic!("expected enemy_overhead_hp_bar at index 2");
        };
        Ok(())
    }
}
