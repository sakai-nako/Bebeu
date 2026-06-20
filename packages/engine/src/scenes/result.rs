//! Result scene (旧 `internal/engine/scenes/result`)。雛形。
use bevy::prelude::*;

use crate::app::SceneState;

pub struct ResultScenePlugin;

impl Plugin for ResultScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(SceneState::Result), setup);
    }
}

fn setup() {
    tracing::info!("result: enter");
}
