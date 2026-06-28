//! Engine ブートストラップ本体と CLI 引数解釈。
//!
//! bin からは [`entrypoint`] を呼べばよい。`entrypoint` が `--project=<name>` /
//! `BEATEMUP_PROJECT` を解釈し、[`run`] に [`RunOptions`] を渡す。
//! どちらも未指定なら convention の `"main"` を default として採用する
//! (実在しなければ `Project::load_from_file` が warn で fail-soft する)。
use std::env;

use anyhow::Result;
use bevy::asset::AssetPlugin;
use bevy::image::ImagePlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::WindowResolution;

use super::pixel_perfect::{PixelPerfectConfig, PixelPerfectRenderPlugin};
use crate::entities::project::Project;
use crate::features::character::{
    AiPlugin, AnimationPlugin, AttackPlugin, DebugControlPlugin, HitStopPlugin, HitboxDebugPlugin,
    KnockbackPlugin, MovementPlugin, SoundPlugin, StateDebugPlugin, StateMachinePlugin,
};
use crate::features::hud::HudPlugin;
use crate::scenes::{battle, options, result, title};
use crate::shared::ActionMap;
use crate::shared::config::{EngineConfig, RuntimePaths, WindowConfig};

/// 既定のログフィルタ。`RUST_LOG` が設定されていればそちらが優先される (Bevy `LogPlugin` 仕様)。
///
/// - `wgpu*` / `naga` は warn 以上 (Vulkan validation layer 未導入の警告などを抑制)
/// - その他は info 既定
const DEFAULT_LOG_FILTER: &str = "wgpu=error,wgpu_core=error,wgpu_hal=error,naga=warn,info";

/// `bebeu-engine.yml` で `window:` が未指定のとき viewport にかける整数倍率。
/// 3 のとき 384×216 → 1152×648 (フル HD に余裕で乗る大きさ)。
/// yml で明示指定された場合はそちらが優先される。
const WINDOW_INTEGER_SCALE_FALLBACK: u32 = 3;
/// Project 未指定時に使う viewport 解像度の fallback (= main project の resolution)。
const FALLBACK_VIEWPORT: (u32, u32) = (384, 216);

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// 起動時にロードする Project 名 (`data/projects/{name}.yml`)。
    /// `entrypoint()` 経由なら未指定時に `"main"` が入る (convention)。
    pub project_name: Option<String>,
}

/// 旧 `Game.Scene` の役割を Bevy [`States`] で表現する。
///
/// 遷移 (Phase 3):
/// - `Title` ↔ `Options` (タイトルメニューからの設定画面、Cancel で戻る)
/// - `Title` → `Battle` (Start 選択)
/// - `Battle` → `Result` (既存、勝敗時)
#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum SceneState {
    #[default]
    Title,
    Options,
    Battle,
    Result,
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
    // `data/projects/main.yml` を default convention とする。`runtime/` と
    // `sample-projects/minimal/` のいずれもこの名前なので、`just engine-run` /
    // `just engine-run-sample` を引数なしで叩いても project が読まれる。
    // 実在しない workspace では `run` 内の `Project::load_from_file` が warn で fail-soft する。
    let project_name = project_name.or_else(|| Some("main".to_owned()));
    run(RunOptions { project_name })
}

/// engine の各 Plugin と `SceneState` を `app` に組み込む。GUI / Asset / Log といった
/// Bevy 標準 plugin は呼び出し側が用意する (本番は `DefaultPlugins`、smoke test は
/// `MinimalPlugins + AssetPlugin`)。
///
/// 戻り値は `&mut App` で chain 可能。
pub fn register_engine_plugins(app: &mut App) -> &mut App {
    app.init_state::<SceneState>()
        .init_resource::<ActionMap>()
        .add_plugins(AnimationPlugin)
        .add_plugins(AiPlugin)
        .add_plugins(MovementPlugin)
        .add_plugins(StateMachinePlugin)
        .add_plugins(AttackPlugin)
        .add_plugins(SoundPlugin)
        .add_plugins(HitStopPlugin)
        .add_plugins(KnockbackPlugin)
        .add_plugins(HitboxDebugPlugin)
        .add_plugins(StateDebugPlugin)
        .add_plugins(DebugControlPlugin)
        .add_plugins(HudPlugin)
        .add_plugins(PixelPerfectRenderPlugin)
        .add_plugins(title::TitleScenePlugin)
        .add_plugins(options::OptionsScenePlugin)
        .add_plugins(battle::BattleScenePlugin)
        .add_plugins(result::ResultScenePlugin)
}

pub fn run(opts: RunOptions) -> Result<()> {
    tracing::info!(?opts, "engine: starting");

    let engine_config = EngineConfig::load();
    let runtime = RuntimePaths::resolve(&engine_config);
    tracing::info!(runtime_root = %runtime.root().display(), "engine: runtime resolved");

    let asset_root = runtime.data_dir().to_string_lossy().into_owned();

    // Window 解像度を決めるため Project を先読み (WindowPlugin に project.resolution × N を渡す)。
    // Project 未指定 or 読み込み失敗時は fallback resolution を使う。
    let project = opts.project_name.as_deref().and_then(|name| {
        let path = runtime.project_file(name);
        match Project::load_from_file(&path, name) {
            Ok(p) => {
                tracing::info!(
                    project = %p.name,
                    players = ?p.players,
                    opponents = ?p.opponents,
                    levels = ?p.levels,
                    "engine: project loaded",
                );
                Some(p)
            }
            Err(err) => {
                tracing::warn!(error = %err, "engine: project load failed, continuing without it");
                None
            }
        }
    });
    if opts.project_name.is_none() {
        tracing::info!("engine: no project specified (use --project=<name> or BEATEMUP_PROJECT)");
    }
    let (vp_w, vp_h) = project.as_ref().map_or(FALLBACK_VIEWPORT, |p| {
        (p.resolution.width, p.resolution.height)
    });
    let (win_w, win_h) = engine_config.window.map_or_else(
        || {
            (
                vp_w * WINDOW_INTEGER_SCALE_FALLBACK,
                vp_h * WINDOW_INTEGER_SCALE_FALLBACK,
            )
        },
        |WindowConfig { width, height }| (width, height),
    );
    let pixel_perfect_config =
        PixelPerfectConfig::from_viewport_and_window((vp_w, vp_h), (win_w, win_h));
    tracing::info!(
        viewport = ?pixel_perfect_config.viewport,
        intermediate = ?pixel_perfect_config.intermediate,
        window = ?pixel_perfect_config.window,
        "engine: pixel-perfect 3-tier sizes resolved (window=yml or viewport×{} fallback)",
        WINDOW_INTEGER_SCALE_FALLBACK,
    );

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Bebeu".into(),
                    // 物理 pixel で固定 + scale_factor_override(1.0) で OS DPI スケーリングを
                    // 無視し、yml と実際の window サイズを 1:1 に保証する。
                    resolution: WindowResolution::new(win_w, win_h).with_scale_factor_override(1.0),
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
            })
            // sprite テクスチャ (キャラ / 背景) はすべて nearest sampling で扱う。
            // 中間 render texture だけ pixel_perfect.rs で linear に上書きする (ADR-0026)。
            .set(ImagePlugin::default_nearest()),
    )
    .insert_resource(pixel_perfect_config);
    register_engine_plugins(&mut app);

    // ActionMap は smoke test では register_engine_plugins の init_resource (Default) で十分。
    // 本番 run では yml override (env > manifest/config/input.yml) を上に被せる。
    app.insert_resource(ActionMap::load());

    if let Some(project) = project {
        app.insert_resource(project);
    }

    app.insert_resource(runtime).run();

    Ok(())
}
