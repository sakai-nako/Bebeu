//! Title scene。`Start` / `Options` / `Quit` の 3 項目メニューを Text2d で表示し、
//! Up/Down で選択、Confirm で確定、Cancel で何もしない (タイトル自体は最上位なので)。
//!
//! UI は state_debug.rs と同じく `FINAL_PASS_LAYER` (= window 解像度) に乗せる。
//! Bevy UI ではなく Text2d を採用したのは:
//! - Camera への bind 設定が不要 (RenderLayers だけで描き分けられる)
//! - 3 項目程度なら手動 transform 配置でも十分シンプル
//! - 既存 state_debug.rs と同じ機構で取り回しが揃う
//!
//! Phase 4 でキーコンフィグ UI を作る段階で Bevy UI 導入の要否を再評価する。
use bevy::app::AppExit;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::app::{FINAL_PASS_LAYER, SceneState};
use crate::shared::{ActionMap, MenuAction, menu_action_just_pressed};

/// メニュー項目総数。Up/Down の wrap 計算に使う。
const MENU_ITEM_COUNT: usize = 3;
/// 非選択中のテキスト色 (淡い灰)。
const COLOR_NORMAL: Color = Color::srgb(0.6, 0.6, 0.6);
/// 選択中のテキスト色 (state_debug と揃えた緑系)。
const COLOR_SELECTED: Color = Color::srgb(0.3, 1.0, 0.4);
/// 項目間の y 方向間隔 (window pixel)。
const MENU_ITEM_SPACING_Y: f32 = 30.0;
/// タイトル文字の y 位置 (window pixel, +Y = 上)。
const TITLE_Y: f32 = 100.0;
/// メニュー項目開始 y (1 項目目の位置)。
const MENU_START_Y: f32 = 0.0;
const TITLE_FONT_SIZE: f32 = 32.0;
const MENU_FONT_SIZE: f32 = 20.0;

/// 現在ハイライト中のメニュー項目 index。Bevy の `Changed` を効かせるため
/// is_changed フックで visuals 更新を走らせる。
#[derive(Resource, Debug, Default)]
struct TitleMenuSelection(usize);

/// OnExit(Title) で despawn する対象 marker。setup で spawn した entity 全てに付ける。
#[derive(Component)]
struct TitleSceneEntity;

/// 各メニュー項目に index を埋めておく (update_selection_visuals での色塗り判定用)。
#[derive(Component)]
struct TitleMenuItem(usize);

pub struct TitleScenePlugin;

impl Plugin for TitleScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TitleMenuSelection>()
            .add_systems(OnEnter(SceneState::Title), setup)
            .add_systems(
                Update,
                (handle_navigation, handle_confirm, update_selection_visuals)
                    .chain()
                    .run_if(in_state(SceneState::Title)),
            )
            .add_systems(OnExit(SceneState::Title), cleanup);
    }
}

fn setup(mut commands: Commands, mut selection: ResMut<TitleMenuSelection>) {
    // OnEnter のたびに先頭項目に戻す (Options から Cancel で帰ってきたとき等)。
    selection.0 = 0;
    tracing::info!("title: enter");
    commands.spawn((
        Text2d::new("Bebeu"),
        TextFont {
            font_size: TITLE_FONT_SIZE,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, TITLE_Y, 100.0),
        RenderLayers::layer(FINAL_PASS_LAYER),
        TitleSceneEntity,
    ));
    for (i, label) in ["Start", "Options", "Quit"].into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = MENU_START_Y - (i as f32) * MENU_ITEM_SPACING_Y;
        let color = if i == 0 { COLOR_SELECTED } else { COLOR_NORMAL };
        commands.spawn((
            Text2d::new(label),
            TextFont {
                font_size: MENU_FONT_SIZE,
                ..default()
            },
            TextColor(color),
            Transform::from_xyz(0.0, y, 100.0),
            RenderLayers::layer(FINAL_PASS_LAYER),
            TitleSceneEntity,
            TitleMenuItem(i),
        ));
    }
}

fn handle_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    action_map: Res<ActionMap>,
    mut selection: ResMut<TitleMenuSelection>,
) {
    if menu_action_just_pressed(&keys, &action_map, MenuAction::Up) {
        selection.0 = (selection.0 + MENU_ITEM_COUNT - 1) % MENU_ITEM_COUNT;
    }
    if menu_action_just_pressed(&keys, &action_map, MenuAction::Down) {
        selection.0 = (selection.0 + 1) % MENU_ITEM_COUNT;
    }
}

fn handle_confirm(
    keys: Res<ButtonInput<KeyCode>>,
    action_map: Res<ActionMap>,
    selection: Res<TitleMenuSelection>,
    mut next: ResMut<NextState<SceneState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !menu_action_just_pressed(&keys, &action_map, MenuAction::Confirm) {
        return;
    }
    match selection.0 {
        0 => next.set(SceneState::Battle),
        1 => next.set(SceneState::Options),
        2 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

fn update_selection_visuals(
    selection: Res<TitleMenuSelection>,
    mut query: Query<(&TitleMenuItem, &mut TextColor)>,
) {
    if !selection.is_changed() {
        return;
    }
    for (item, mut color) in &mut query {
        color.0 = if item.0 == selection.0 {
            COLOR_SELECTED
        } else {
            COLOR_NORMAL
        };
    }
}

fn cleanup(mut commands: Commands, query: Query<Entity, With<TitleSceneEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Up/Down の wrap 計算 (pure)。`handle_navigation` から取り出した算術部分を直接検証。
    fn next_up(current: usize) -> usize {
        (current + MENU_ITEM_COUNT - 1) % MENU_ITEM_COUNT
    }
    fn next_down(current: usize) -> usize {
        (current + 1) % MENU_ITEM_COUNT
    }

    #[test]
    fn navigation_down_wraps_at_last_to_first() {
        assert_eq!(next_down(0), 1);
        assert_eq!(next_down(1), 2);
        assert_eq!(next_down(MENU_ITEM_COUNT - 1), 0);
    }

    #[test]
    fn navigation_up_wraps_at_first_to_last() {
        assert_eq!(next_up(2), 1);
        assert_eq!(next_up(1), 0);
        assert_eq!(next_up(0), MENU_ITEM_COUNT - 1);
    }
}
