//! Title scene (旧 `internal/engine/scenes/title`)。雛形。
use bevy::prelude::*;

use crate::app::SceneState;

pub struct TitleScenePlugin;

impl Plugin for TitleScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(SceneState::Title), setup)
            .add_systems(Update, advance.run_if(in_state(SceneState::Title)));
    }
}

fn setup() {
    tracing::info!("title: enter");
}

fn advance(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<SceneState>>,
) {
    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space) {
        next.set(SceneState::Battle);
    }
}
