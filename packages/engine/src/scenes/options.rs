//! Options scene — gameplay action のキーコンフィグ画面 (Phase 4)。
//!
//! 機能:
//! - 8 Action (MoveLeft / MoveRight / ... / Guard) のバインドを表示
//! - 項目選択 (Up/Down) → Confirm で **編集モード** に入り、押された KeyCode で置換
//!   (1 Action = 1 KeyCode の単純化、複数 bind は yml 直編集に委ねる)
//! - 「Reset to default」で `ActionMap::default` のバインドに戻す (まだ commit はされない)
//! - 「Back」で **編集中の bind を `ActionMap` に commit + `input.yml` に save** して Title へ
//! - Browse 中の Esc は **編集を破棄** して Title へ (= `EditingActionMap` は捨てる)
//! - 編集中の Esc は **キャプチャだけ抜ける** (binding は変えない)
//!
//! 設計上の Resource:
//! - [`EditingActionMap`] — Options scene 内でだけ生きる、編集中のバインド。Browse / 編集 /
//!   Reset で書き換わり、Back で `ActionMap` (Resource) に commit される。
//! - [`KeyConfigMode`] — `Browse` か `EditingAction(Action)` の二値。enum 状態。
//! - [`OptionsMenuSelection`] — 現在ハイライト中の行 index (0..[`MENU_ITEM_COUNT`])。
//!
//! UI は state_debug / title と同じ `FINAL_PASS_LAYER` (= window 解像度) に Text2d で出す。
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::app::{FINAL_PASS_LAYER, SceneState};
use crate::shared::{
    Action, ActionMap, MenuAction, key_code_to_str, menu_action_just_pressed, write_input_yml,
};

/// 全 10 行: 8 Action + Reset + Back。
const MENU_ITEM_COUNT: usize = Action::ALL.len() + 2;
/// Reset 行の index ([`Action::ALL`] の直後)。
const RESET_INDEX: usize = Action::ALL.len();
/// Back 行の index ([`RESET_INDEX`] の直後)。
const BACK_INDEX: usize = RESET_INDEX + 1;

const COLOR_NORMAL: Color = Color::srgb(0.6, 0.6, 0.6);
const COLOR_SELECTED: Color = Color::srgb(0.3, 1.0, 0.4);
/// 編集モード中の現在行 (yellow) + Press Any Key prompt の色。
const COLOR_EDITING: Color = Color::srgb(1.0, 0.85, 0.2);
const HEADING_COLOR: Color = Color::WHITE;

const HEADING_Y: f32 = 130.0;
const HEADING_FONT_SIZE: f32 = 28.0;
const ROW_FONT_SIZE: f32 = 16.0;
const PROMPT_FONT_SIZE: f32 = 14.0;
/// 各行の y 間隔 (px)。10 行で ±100 px 程度に収まるよう 22 px に。
const ROW_SPACING_Y: f32 = 22.0;
/// 1 行目 (i=0) の y 位置 (px)。
const ROW_START_Y: f32 = 80.0;
/// Press Any Key prompt の y 位置 (画面下寄せ)。
const PROMPT_Y: f32 = -190.0;
/// 左カラム (Action 名) の左端 x 位置 (Text2d を左揃え anchor で配置)。
const COL_LABEL_X: f32 = -140.0;
/// 右カラム (バインド表示) の左端 x 位置。Action 名最長 (`down_attack`) と + 余白で決定。
const COL_BINDING_X: f32 = -30.0;

/// この scene が生存中だけ存在する、編集中の [`ActionMap`]。OnEnter で `ActionMap`
/// のクローン、OnExit で remove。Back で `ActionMap` に commit される。
#[derive(Resource, Debug, Clone)]
struct EditingActionMap(ActionMap);

/// キーコンフィグ画面の入力モード。
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
enum KeyConfigMode {
    /// 行選択中。Up/Down で移動、Confirm で行に応じた action を実行。
    #[default]
    Browse,
    /// この Action のバインドを編集中。次に押された有効 KeyCode で置換 → Browse に戻る。
    EditingAction(Action),
}

/// 現在ハイライト中の行 index。
#[derive(Resource, Debug, Default)]
struct OptionsMenuSelection(usize);

#[derive(Component)]
struct OptionsSceneEntity;

/// 各メニュー行 marker — 色塗りと「どの行か」の判定に使う。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum OptionsMenuRow {
    Action(Action),
    Reset,
    Back,
}

impl OptionsMenuRow {
    /// この行の selection 上の index。
    fn index(self) -> usize {
        match self {
            Self::Action(a) => action_index(a),
            Self::Reset => RESET_INDEX,
            Self::Back => BACK_INDEX,
        }
    }
}

/// 右カラム (バインド表示) の Text2d marker。Action ごとに 1 つ存在。
#[derive(Component)]
struct OptionsBindingLabel(Action);

/// 編集モード中だけ Visible にする「Press any key...」prompt。
#[derive(Component)]
struct PressAnyKeyPrompt;

/// `Action::ALL` 内の位置 (selection index と一致)。
fn action_index(action: Action) -> usize {
    Action::ALL
        .iter()
        .position(|&a| a == action)
        .expect("Action::ALL covers all variants")
}

/// selection index から `OptionsMenuRow` を逆引き。
fn row_at_index(index: usize) -> Option<OptionsMenuRow> {
    if let Some(action) = Action::ALL.get(index).copied() {
        return Some(OptionsMenuRow::Action(action));
    }
    match index {
        RESET_INDEX => Some(OptionsMenuRow::Reset),
        BACK_INDEX => Some(OptionsMenuRow::Back),
        _ => None,
    }
}

pub struct OptionsScenePlugin;

impl Plugin for OptionsScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KeyConfigMode>()
            .init_resource::<OptionsMenuSelection>()
            .add_systems(OnEnter(SceneState::Options), setup)
            .add_systems(
                Update,
                (
                    handle_key_capture,
                    handle_navigation,
                    handle_confirm,
                    handle_cancel,
                    update_binding_labels,
                    update_row_visuals,
                    update_prompt_visibility,
                )
                    .chain()
                    .run_if(in_state(SceneState::Options)),
            )
            .add_systems(OnExit(SceneState::Options), cleanup);
    }
}

fn setup(mut commands: Commands, action_map: Res<ActionMap>) {
    tracing::info!("options: enter");
    commands.insert_resource(EditingActionMap(action_map.clone()));
    commands.insert_resource(KeyConfigMode::default());
    commands.insert_resource(OptionsMenuSelection(0));

    commands.spawn((
        Text2d::new("Key Config"),
        TextFont {
            font_size: HEADING_FONT_SIZE,
            ..default()
        },
        TextColor(HEADING_COLOR),
        Transform::from_xyz(0.0, HEADING_Y, 100.0),
        RenderLayers::layer(FINAL_PASS_LAYER),
        OptionsSceneEntity,
    ));

    // Action 行: 左に名前、右にバインド一覧。
    for (i, action) in Action::ALL.into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = ROW_START_Y - (i as f32) * ROW_SPACING_Y;
        commands.spawn((
            Text2d::new(action.as_snake_case()),
            TextFont {
                font_size: ROW_FONT_SIZE,
                ..default()
            },
            TextColor(COLOR_NORMAL),
            // 左揃え anchor (x=-0.5 で左端が中心)。左カラムが文字長で揃う。
            Anchor(Vec2::new(-0.5, 0.0)),
            Transform::from_xyz(COL_LABEL_X, y, 100.0),
            RenderLayers::layer(FINAL_PASS_LAYER),
            OptionsSceneEntity,
            OptionsMenuRow::Action(action),
        ));
        commands.spawn((
            Text2d::new(format_bindings(&action_map, action)),
            TextFont {
                font_size: ROW_FONT_SIZE,
                ..default()
            },
            TextColor(COLOR_NORMAL),
            Anchor(Vec2::new(-0.5, 0.0)),
            Transform::from_xyz(COL_BINDING_X, y, 100.0),
            RenderLayers::layer(FINAL_PASS_LAYER),
            OptionsSceneEntity,
            OptionsBindingLabel(action),
        ));
    }

    // Reset / Back 行は中央配置。
    #[allow(clippy::cast_precision_loss)]
    let reset_y = ROW_START_Y - (RESET_INDEX as f32) * ROW_SPACING_Y - ROW_SPACING_Y * 0.5;
    commands.spawn((
        Text2d::new("Reset to default"),
        TextFont {
            font_size: ROW_FONT_SIZE,
            ..default()
        },
        TextColor(COLOR_NORMAL),
        Transform::from_xyz(0.0, reset_y, 100.0),
        RenderLayers::layer(FINAL_PASS_LAYER),
        OptionsSceneEntity,
        OptionsMenuRow::Reset,
    ));
    #[allow(clippy::cast_precision_loss)]
    let back_y = reset_y - ROW_SPACING_Y;
    commands.spawn((
        Text2d::new("Back (save & return)"),
        TextFont {
            font_size: ROW_FONT_SIZE,
            ..default()
        },
        TextColor(COLOR_NORMAL),
        Transform::from_xyz(0.0, back_y, 100.0),
        RenderLayers::layer(FINAL_PASS_LAYER),
        OptionsSceneEntity,
        OptionsMenuRow::Back,
    ));

    // Press any key prompt — 編集モード中だけ可視。
    commands.spawn((
        Text2d::new("Press any key to bind, Esc to cancel"),
        TextFont {
            font_size: PROMPT_FONT_SIZE,
            ..default()
        },
        TextColor(COLOR_EDITING),
        Transform::from_xyz(0.0, PROMPT_Y, 100.0),
        Visibility::Hidden,
        RenderLayers::layer(FINAL_PASS_LAYER),
        OptionsSceneEntity,
        PressAnyKeyPrompt,
    ));
}

/// `[ArrowLeft, KeyA]` のような表示文字列を組み立てる。空ならプレースホルダ `(none)`。
fn format_bindings(map: &ActionMap, action: Action) -> String {
    let names: Vec<&'static str> = map
        .bindings_for(action)
        .iter()
        .filter_map(|&c| key_code_to_str(c))
        .collect();
    if names.is_empty() {
        "(none)".to_string()
    } else {
        format!("[{}]", names.join(", "))
    }
}

fn handle_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    action_map: Res<ActionMap>,
    mode: Res<KeyConfigMode>,
    mut selection: ResMut<OptionsMenuSelection>,
) {
    if *mode != KeyConfigMode::Browse {
        return;
    }
    if menu_action_just_pressed(&keys, &action_map, MenuAction::Up) {
        selection.0 = (selection.0 + MENU_ITEM_COUNT - 1) % MENU_ITEM_COUNT;
    }
    if menu_action_just_pressed(&keys, &action_map, MenuAction::Down) {
        selection.0 = (selection.0 + 1) % MENU_ITEM_COUNT;
    }
}

#[allow(clippy::needless_pass_by_value)]
fn handle_confirm(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<OptionsMenuSelection>,
    mut mode: ResMut<KeyConfigMode>,
    mut editing: ResMut<EditingActionMap>,
    mut action_map: ResMut<ActionMap>,
    mut next: ResMut<NextState<SceneState>>,
) {
    if *mode != KeyConfigMode::Browse {
        return;
    }
    // 同フレームで mode が Browse に変わった直後 (= handle_key_capture が Enter/Space を
    // bind して edit mode を抜けた直後) は Confirm 入力を消費しない。これをしないと
    // 「Enter キーを bind した瞬間に同フレームで edit mode に再突入」が起きる。
    if mode.is_changed() {
        return;
    }
    if !menu_action_just_pressed(&keys, &action_map, MenuAction::Confirm) {
        return;
    }
    let Some(row) = row_at_index(selection.0) else {
        return;
    };
    match row {
        OptionsMenuRow::Action(action) => {
            tracing::info!(?action, "options: enter edit mode");
            *mode = KeyConfigMode::EditingAction(action);
        }
        OptionsMenuRow::Reset => {
            editing.0 = ActionMap::default();
            tracing::info!("options: editing map reset to defaults");
        }
        OptionsMenuRow::Back => {
            // 編集中のバインドを ActionMap (Resource) に commit し、yml にも永続化。
            // 次フレームから gameplay system が新しい bind で動く。
            commit_to_action_map_and_save(&editing.0, &mut action_map);
            next.set(SceneState::Title);
        }
    }
}

fn handle_cancel(
    keys: Res<ButtonInput<KeyCode>>,
    action_map: Res<ActionMap>,
    mut mode: ResMut<KeyConfigMode>,
    mut next: ResMut<NextState<SceneState>>,
) {
    // handle_confirm と同じく、同フレーム mode 遷移直後は Cancel 入力を消費しない。
    // (Jump キーで bind 直後に同フレーム Cancel = Title 戻りの誤発火を避ける。)
    if mode.is_changed() {
        return;
    }
    // edit mode 中は固定キー Esc だけ受ける (gameplay Jump bind は capture 側に消費させる)。
    let cancelled = match *mode {
        KeyConfigMode::Browse => menu_action_just_pressed(&keys, &action_map, MenuAction::Cancel),
        KeyConfigMode::EditingAction(_) => keys.just_pressed(KeyCode::Escape),
    };
    if !cancelled {
        return;
    }
    match *mode {
        KeyConfigMode::Browse => {
            tracing::info!("options: cancel from browse, discarding edits");
            next.set(SceneState::Title);
        }
        KeyConfigMode::EditingAction(_) => {
            tracing::info!("options: cancel from edit mode, binding unchanged");
            *mode = KeyConfigMode::Browse;
        }
    }
}

/// 編集モード中: 押された KeyCode のうち最初に [`key_code_to_str`] で名前を引けたものを
/// 編集対象 Action のバインドに置く (= 1 Action = 1 KeyCode に置換)。それから Browse モードに戻る。
/// menu キー (Enter/Space/Esc/Up/Down) も capture 対象 — もしユーザが「Enter を Jump に
/// 割り当てたい」と思ったら Browse 中の Confirm で Edit に入り、改めて Enter を押せば bind 可能。
/// Esc は [`handle_cancel`] が先に処理して edit を抜けるので、bind 候補にはならない。
#[allow(clippy::needless_pass_by_value)]
fn handle_key_capture(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<KeyConfigMode>,
    mut editing: ResMut<EditingActionMap>,
) {
    let KeyConfigMode::EditingAction(action) = *mode else {
        return;
    };
    // Esc は handle_cancel に任せたいので、capture からは除外する (= edit を抜けるだけ)。
    for &code in keys.get_just_pressed() {
        if code == KeyCode::Escape {
            continue;
        }
        let Some(name) = key_code_to_str(code) else {
            // unsupported (Numpad 系等): skip。何も起きなかったように見えるが、edit モードは
            // 解除しない (= 次の有効キー入力を待つ)。
            tracing::debug!(?code, "options: unsupported key ignored in capture");
            continue;
        };
        editing.0.set_binding(action, vec![code]);
        tracing::info!(?action, key = name, "options: bind updated");
        *mode = KeyConfigMode::Browse;
        return;
    }
}

fn update_binding_labels(
    editing: Res<EditingActionMap>,
    mut query: Query<(&OptionsBindingLabel, &mut Text2d)>,
) {
    if !editing.is_changed() {
        return;
    }
    for (label, mut text) in &mut query {
        text.0 = format_bindings(&editing.0, label.0);
    }
}

fn update_row_visuals(
    selection: Res<OptionsMenuSelection>,
    mode: Res<KeyConfigMode>,
    mut query: Query<(&OptionsMenuRow, &mut TextColor)>,
) {
    if !selection.is_changed() && !mode.is_changed() {
        return;
    }
    for (row, mut color) in &mut query {
        color.0 = if row.index() == selection.0 {
            match *mode {
                KeyConfigMode::EditingAction(action) if OptionsMenuRow::Action(action) == *row => {
                    COLOR_EDITING
                }
                _ => COLOR_SELECTED,
            }
        } else {
            COLOR_NORMAL
        };
    }
}

fn update_prompt_visibility(
    mode: Res<KeyConfigMode>,
    mut query: Query<&mut Visibility, With<PressAnyKeyPrompt>>,
) {
    if !mode.is_changed() {
        return;
    }
    let visible = matches!(*mode, KeyConfigMode::EditingAction(_));
    for mut vis in &mut query {
        *vis = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Back 確定時に `ActionMap` (Resource) を編集後のバインドで上書きし、`input.yml` にも save する。
/// Cancel ルート (`handle_cancel`) からは呼ばれないため、Cancel 後の編集は「破棄」扱い。
fn commit_to_action_map_and_save(editing: &ActionMap, action_map: &mut ActionMap) {
    *action_map = editing.clone();
    let path = ActionMap::default_yml_path();
    match write_input_yml(&path, editing) {
        Ok(()) => tracing::info!(path = %path.display(), "options: committed + saved input.yml"),
        Err(err) => {
            tracing::warn!(error = %err, path = %path.display(), "options: save failed");
        }
    }
}

fn cleanup(mut commands: Commands, query: Query<Entity, With<OptionsSceneEntity>>) {
    // ActionMap への commit と yml save は Back 経路 (commit_to_action_map_and_save) で
    // 既に済んでいる。Cancel 経路は EditingActionMap を捨てるだけで、ActionMap は変更されない。
    commands.remove_resource::<EditingActionMap>();
    commands.insert_resource(KeyConfigMode::default());
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
