//! Engine ブートストラップ本体と CLI 引数解釈。
//!
//! bin からは [`entrypoint`] を呼べばよい。`entrypoint` が `--project=<name>` /
//! `BEATEMUP_PROJECT` を解釈し、[`run`] に [`RunOptions`] を渡す。
use std::env;

use anyhow::Result;
use bevy::asset::AssetPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;

use crate::entities::project::Project;
use crate::features::character::{AnimationPlugin, MovementPlugin, StateMachinePlugin};
use crate::scenes::{battle, result, title};
use crate::shared::config::RuntimePaths;

/// 既定のログフィルタ。`RUST_LOG` が設定されていればそちらが優先される (Bevy `LogPlugin` 仕様)。
///
/// - `wgpu*` / `naga` は warn 以上 (Vulkan validation layer 未導入の警告などを抑制)
/// - その他は info 既定
const DEFAULT_LOG_FILTER: &str = "wgpu=error,wgpu_core=error,wgpu_hal=error,naga=warn,info";

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// 起動時にロードする Project 名 (`data/projects/{name}.yml`)。
    pub project_name: Option<String>,
}

/// 旧 `Game.Scene` の役割を Bevy [`States`] で表現する。
#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum SceneState {
    #[default]
    Title,
    Battle,
    Result,
}

/// 起動時オプションを Resource として持ち回す。
#[derive(Resource, Debug, Clone, Default)]
pub struct EngineConfig {
    pub project_name: Option<String>,
}

/// CLI 引数と環境変数を解釈して [`run`] を呼ぶ。bin からはこれだけを呼べばよい。
pub fn entrypoint() -> Result<()> {
    let mut project_name: Option<String> = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => project_name = args.next(),
            other if other.starts_with("--project=") => {
                project_name = Some(other.trim_start_matches("--project=").to_owned());
            }
            _ => {}
        }
    }
    if project_name.is_none() {
        project_name = env::var("BEATEMUP_PROJECT").ok().filter(|s| !s.is_empty());
    }
    run(RunOptions { project_name })
}

pub fn run(opts: RunOptions) -> Result<()> {
    tracing::info!(?opts, "engine: starting");

    let runtime = RuntimePaths::resolve();
    tracing::info!(runtime_root = %runtime.root().display(), "engine: runtime resolved");

    let asset_root = runtime.data_dir().to_string_lossy().into_owned();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "beatemup".into(),
                    ..default()
                }),
                ..default()
            })
            .set(LogPlugin {
                filter: DEFAULT_LOG_FILTER.into(),
                level: bevy::log::Level::INFO,
                ..default()
            })
            .set(AssetPlugin {
                file_path: asset_root,
                ..default()
            }),
    )
    .insert_resource(EngineConfig { project_name: opts.project_name.clone() })
    .init_state::<SceneState>()
    .add_plugins(AnimationPlugin)
    .add_plugins(MovementPlugin)
    .add_plugins(StateMachinePlugin)
    .add_plugins(title::TitleScenePlugin)
    .add_plugins(battle::BattleScenePlugin)
    .add_plugins(result::ResultScenePlugin);

    if let Some(name) = opts.project_name.as_deref() {
        let path = runtime.project_file(name);
        match Project::load_from_file(&path, name) {
            Ok(project) => {
                tracing::info!(
                    project = %project.name,
                    players = ?project.players,
                    opponents = ?project.opponents,
                    levels = ?project.levels,
                    "engine: project loaded",
                );
                app.insert_resource(project);
            }
            Err(err) => {
                tracing::warn!(error = %err, "engine: project load failed, continuing without it");
            }
        }
    } else {
        tracing::info!("engine: no project specified (use --project=<name> or BEATEMUP_PROJECT)");
    }

    app.insert_resource(runtime).run();

    Ok(())
}
