//! engine の全 Plugin を組み合わせて headless で 1 frame 回す smoke test。
//!
//! 目的は Bevy の SystemParam 衝突 (B0001) や schedule build エラーといった
//! **起動時に panic する系の誤り** を `just verify` の中で自動 catch すること。
//! 実 asset / window / render は要らないので、`MinimalPlugins` + `AssetPlugin` だけで
//! 立ち上げて [`engine::register_engine_plugins`] を被せる。Project / Level Resource は
//! 注入しないので battle scene は OnEnter まで行かず、title scene の Update が回るだけ。
//! それでも `Update` に登録された全 system (attack / hitbox_debug / movement 等) は走り、
//! SystemParam の access 検証はかかる。

use bevy::asset::AssetPlugin;
use bevy::gizmos::GizmoPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

#[test]
fn engine_app_runs_a_few_frames_without_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    // engine 内で asset_server.load::<Image>(...) を呼ぶ system があるため Image asset の
    // 登録が必要。未登録だと AssetServer の access で panic する。
    app.init_asset::<Image>();
    // engine の SceneState を init_state する前提条件。本番は DefaultPlugins に含まれるが
    // MinimalPlugins には無いので smoke test で明示追加する。
    app.add_plugins(StatesPlugin);
    // handle_input / toggle_debug / title::advance が `Res<ButtonInput<KeyCode>>` を取る。
    // InputPlugin が無いと「Resource does not exist」で panic する。
    app.add_plugins(InputPlugin);
    // hitbox_debug::draw_hitboxes が `Gizmos` を取るため GizmoPlugin が要る。
    app.add_plugins(GizmoPlugin);
    engine::register_engine_plugins(&mut app);

    // 数 frame 回して Startup / Update / state 遷移系の system params を一通り fetch させる。
    // 1 回でも B0001 は捕まるが、複数 frame 回す方が `Changed<>` 系も活性化する。
    for _ in 0..3 {
        app.update();
    }
}
