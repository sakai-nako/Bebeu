//! Engine ブートストラップ本体と CLI 引数解釈。
//!
//! bin からは [`entrypoint`] を呼べばよい。`entrypoint` が `--project=<name>` /
//! `BEATEMUP_PROJECT` を解釈し、[`run`] に [`RunOptions`] を渡す。
//! どちらも未指定なら convention の `"main"` を default として採用する
//! (実在しなければ `Project::load_from_file` が warn で fail-soft する)。
use std::env;
use std::time::Duration;

use anyhow::Result;
use bevy::asset::AssetPlugin;
use bevy::image::ImagePlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::settings::{SaveSettingsDeferred, SaveSettingsSync, SettingsPlugin};
use bevy::window::{
    ExitCondition, MonitorSelection, PrimaryWindow, WindowCloseRequested, WindowMode, WindowMoved,
    WindowPosition, WindowResized, WindowResolution,
};

use super::pixel_perfect::{PixelPerfectConfig, PixelPerfectRenderPlugin};
use crate::entities::project::Project;
use crate::features::character::{
    AiPlugin, AnimationPlugin, AttackPlugin, DebugControlPlugin, HitStopPlugin, HitboxDebugPlugin,
    KnockbackPlugin, MovementPlugin, SoundPlugin, StateDebugPlugin, StateMachinePlugin,
};
use crate::features::hud::HudPlugin;
use crate::scenes::{battle, options, result, title};
use crate::shared::ActionMap;
use crate::shared::config::{EngineConfig, RuntimePaths};
use crate::shared::settings::{APP_NAME, WindowSettings};

/// 既定のログフィルタ。`RUST_LOG` が設定されていればそちらが優先される (Bevy `LogPlugin` 仕様)。
///
/// - `wgpu*` / `naga` は warn 以上 (Vulkan validation layer 未導入の警告などを抑制)
/// - その他は info 既定
const DEFAULT_LOG_FILTER: &str = "wgpu=error,wgpu_core=error,wgpu_hal=error,naga=warn,info";

/// viewport にかける整数倍率 (初期 window サイズ算出用)。3 のとき 384×216 → 1152×648。
/// ADR-0041 で window サイズは App Settings (`WindowSettings.size`) に移管したが、
/// 初回起動時 (= TOML 不在) はこの定数で算出した値が初期サイズになる。
const WINDOW_INTEGER_SCALE_FALLBACK: u32 = 3;
/// Project 未指定時に使う viewport 解像度の fallback (= main project の resolution)。
const FALLBACK_VIEWPORT: (u32, u32) = (384, 216);

/// `SaveSettingsDeferred` の debounce 遅延 (window move/resize の連続発火を吸収)。
const SETTINGS_SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

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
    // 初期 window サイズは viewport × 整数倍率の固定 fallback。
    // 起動後に `apply_window_settings` で `WindowSettings.size` があれば override される
    // (ADR-0041 — pixel_perfect_config の resize 追従は別 Issue で扱う)。
    let (win_w, win_h) = (
        vp_w * WINDOW_INTEGER_SCALE_FALLBACK,
        vp_h * WINDOW_INTEGER_SCALE_FALLBACK,
    );
    let pixel_perfect_config =
        PixelPerfectConfig::from_viewport_and_window((vp_w, vp_h), (win_w, win_h));
    tracing::info!(
        viewport = ?pixel_perfect_config.viewport,
        intermediate = ?pixel_perfect_config.intermediate,
        window = ?pixel_perfect_config.window,
        "engine: pixel-perfect 3-tier sizes resolved (initial = viewport×{})",
        WINDOW_INTEGER_SCALE_FALLBACK,
    );

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Bebeu".into(),
                    // 物理 pixel で固定 + scale_factor_override(1.0) で OS DPI スケーリングを
                    // 無視し、要求 window サイズと実際の window サイズを 1:1 に保証する。
                    resolution: WindowResolution::new(win_w, win_h).with_scale_factor_override(1.0),
                    ..default()
                }),
                // ADR-0041 — close 直前に `SaveSettingsSync::IfChanged` を queue する余地を
                // 残すため、Bevy デフォルトの「最後の window が閉じたら即 exit」を抑止する。
                exit_condition: ExitCondition::DontExit,
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
    .add_plugins(SettingsPlugin::new(APP_NAME))
    .add_plugins(UserSettingsPlugin)
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

/// ADR-0041 — `WindowSettings` / `AudioSettings` を Window entity に反映 + 変更検出 +
/// close 時 save を担う wiring。`SettingsPlugin` の **後** に追加すること
/// (前置だと `WindowSettings` が世界にまだ無く、apply 関数が no-op になる)。
struct UserSettingsPlugin;

impl Plugin for UserSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, apply_window_settings)
            .add_systems(Update, (track_window_changes, save_on_window_close).chain());
    }
}

/// 起動時、`WindowSettings` の保存値があれば primary window に反映する。
/// 値が `None` のフィールドは触らない (= primary_window の初期値が残る)。
fn apply_window_settings(
    settings: Res<WindowSettings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if let Some(position) = settings.position {
        window.position = WindowPosition::At(position);
    }
    if let Some(size) = settings.size {
        window.resolution = WindowResolution::new(size.x, size.y).with_scale_factor_override(1.0);
    }
    window.mode = if settings.fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };
}

/// ユーザーによる window 移動 / リサイズを検出して `WindowSettings` に反映、debounce 保存。
fn track_window_changes(
    mut moved: MessageReader<WindowMoved>,
    mut resized: MessageReader<WindowResized>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut settings: ResMut<WindowSettings>,
    mut commands: Commands,
) {
    let mut changed = false;
    for _ in moved.read() {
        changed = true;
    }
    for _ in resized.read() {
        changed = true;
    }
    if !changed {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let next = WindowSettings {
        position: match window.position {
            WindowPosition::At(pos) => Some(pos),
            _ => None,
        },
        size: Some(UVec2::new(
            window.resolution.width() as u32,
            window.resolution.height() as u32,
        )),
        fullscreen: window.mode != WindowMode::Windowed,
    };
    if settings.set_if_neq(next) {
        commands.queue(SaveSettingsDeferred(SETTINGS_SAVE_DEBOUNCE));
    }
}

/// `ExitCondition::DontExit` の代わりに、最後の window が閉じたら settings を同期保存し
/// てから明示的に `AppExit` を発行する (ADR-0041)。
fn save_on_window_close(
    mut close: MessageReader<WindowCloseRequested>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if close.read().next().is_some() {
        commands.queue(SaveSettingsSync::IfChanged);
        exit.write(AppExit::Success);
    }
}
