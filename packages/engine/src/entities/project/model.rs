//! Project の型定義 (FSD: model segment)。
//!
//! `runtime/data/projects/{name}.yml` の構造に対応する。
//! ロード処理は隣の [`super::api`] に分離している。
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            width: 384,
            height: 216,
        }
    }
}

/// Project は battle 起動時に「どのキャラとどのレベルを使うか」を束ねた起点データ。
///
/// YAML 側にはファイル名 = `name` のため `name` フィールドは持たない。
/// ロード時にファイル stem を埋める。
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub resolution: Resolution,
    #[serde(default)]
    pub players: Vec<String>,
    #[serde(default)]
    pub opponents: Vec<String>,
    #[serde(default)]
    pub levels: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_default_matches_engine_default() {
        let r = Resolution::default();
        assert_eq!(r.width, 384);
        assert_eq!(r.height, 216);
    }

    #[test]
    fn project_default_is_empty_with_default_resolution() {
        let p = Project::default();
        assert!(p.name.is_empty());
        assert_eq!(p.resolution, Resolution::default());
        assert!(p.players.is_empty());
        assert!(p.opponents.is_empty());
        assert!(p.levels.is_empty());
    }
}
