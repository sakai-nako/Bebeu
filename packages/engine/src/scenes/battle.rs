//! Battle scene。Project の最初の player と Level の背景を描画する。
//!
//! 座標系は ADR-0023 に従い、`shared::projection` 経由で Bevy world に変換する。
//! player は Character::load_directory で sprite-groups と animations を populate し、
//! role=Walk の Animation を選んで `AnimationFrames` を構築する (Phase 3: layer は
//! 各 frame の先頭 1 つだけを使う)。
use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::app::SceneState;
use crate::entities::character::{Animation, Character, Role, SpriteEntry, SpriteGroup};
use crate::entities::level::Level;
use crate::entities::project::Project;
use crate::features::character::{
    AnimationData, AnimationFrames, Facing, FrameRender, MainCamera, Player,
    PlayerAnimationLibrary, PlayerState, WorldPosition,
};
use crate::shared::config::RuntimePaths;
use crate::shared::flip::flip_x_of;
use crate::shared::png_header;
use crate::shared::projection;

/// 背景は手前のキャラより必ず奥に描画する (Bevy z-order: 大が手前)。
/// Camera2d の orthographic projection の near/far は default `±1000` なので、
/// その範囲内に収まる小さな負値を使う。
const BACKGROUND_Z: f32 = -1.0;

pub struct BattleScenePlugin;

impl Plugin for BattleScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(SceneState::Battle), setup);
    }
}

#[allow(clippy::too_many_lines)] // Bevy scene setup は spawn のチェーンで長くなりがち
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    runtime: Res<RuntimePaths>,
    project: Option<Res<Project>>,
) {
    tracing::info!("battle: enter");

    let Some(project) = project else {
        tracing::warn!("battle: no Project resource — running with engine defaults");
        commands.spawn(Camera2d);
        return;
    };

    // Level
    let level = project
        .levels
        .first()
        .and_then(|name| match Level::load_from_file(&runtime.level_file(name), name) {
            Ok(l) => Some(l),
            Err(err) => {
                tracing::warn!(error = %err, level = %name, "battle: level load failed");
                None
            }
        })
        .unwrap_or_else(|| Level::with_defaults("default"));

    tracing::info!(
        level = %level.name,
        player_spawn_x = level.player_spawn_x,
        player_spawn_z = level.player_spawn_z,
        "battle: level loaded",
    );

    // Camera
    let cam_translation = projection::camera_translation(
        level.camera_start_x,
        level.camera_start_y,
        project.resolution,
    );
    commands.spawn((
        Camera2d,
        MainCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: project.resolution.width as f32,
                height: project.resolution.height as f32,
            },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_translation(cam_translation),
    ));

    // Background
    let bg_path = format!("levels/{}/{}", level.name, level.base);
    let bg_image: Handle<Image> = asset_server.load(&bg_path);
    commands.spawn((
        Sprite::from_image(bg_image),
        Anchor::TOP_LEFT,
        Transform::from_xyz(0.0, 0.0, BACKGROUND_Z),
    ));
    tracing::info!(asset = %bg_path, "battle: spawning background");

    // Player
    let Some(player_name) = project.players.first() else {
        tracing::warn!("battle: project has no players");
        return;
    };
    let character = match Character::load_directory(&runtime, player_name) {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(error = %err, "battle: character load failed");
            return;
        }
    };
    tracing::info!(
        character = %character.name,
        hp = character.hp,
        sprite_groups = character.sprite_groups.len(),
        animations = character.animations.len(),
        "battle: character loaded",
    );

    // idle / walk の両 animation を build_animation_frames して library に投入。
    // 起動時にここで PNG header を全部読むので、以降の state 切替は O(1)。
    let mut library = PlayerAnimationLibrary::default();
    for role in [Role::Idle, Role::Walk] {
        let Some(anim) = character.animations.iter().find(|a| a.role == role) else {
            tracing::warn!(?role, "battle: character has no animation for role");
            continue;
        };
        let Some(frames) = build_animation_frames(
            &runtime,
            &asset_server,
            player_name,
            anim,
            &character.sprite_groups,
        ) else {
            continue;
        };
        tracing::info!(
            ?role,
            frame_count = frames.len(),
            is_loop = anim.is_loop,
            "battle: animation registered",
        );
        library.insert(
            role,
            AnimationData {
                frames,
                is_loop: anim.is_loop,
                loop_start_index: anim.loop_start_index as usize,
            },
        );
    }

    // 初期表示は Idle で spawn。state_machine::sync_animation が PlayerState 変化を
    // 検知して以降の hot swap を担当する。
    let Some(initial) = library.get(Role::Idle).cloned() else {
        tracing::warn!("battle: cannot spawn player without idle animation");
        commands.insert_resource(library);
        return;
    };

    let player_translation =
        projection::world_to_bevy_f32(level.player_spawn_x as f32, 0.0, level.player_spawn_z as f32);
    let first_handle = initial.frames[0].handle.clone();
    let first_anchor = initial.frames[0].anchor;
    tracing::info!(
        translation = ?player_translation,
        "battle: spawning player (initial state = Idle)",
    );
    commands.spawn((
        Sprite::from_image(first_handle),
        first_anchor,
        Transform::from_translation(player_translation),
        AnimationFrames::new(
            initial.frames,
            initial.is_loop,
            initial.loop_start_index,
        ),
        Player,
        PlayerState::default(),
        Facing::default(),
        WorldPosition::new(level.player_spawn_x as f32, 0.0, level.player_spawn_z as f32),
    ));

    commands.insert_resource(library);
    // movement::handle_input が areas を読めるよう Resource として注入する。
    commands.insert_resource(level);
}

/// Animation の各 frame について、layer[0] から sprite を解決し、PNG dimensions を
/// 読んで `FrameRender` を構築する。Phase 4 では各 frame の layer は先頭 1 つだけを
/// 使い、frame.flip XOR layer.flip を flip_x に焼き、layer.transparency を alpha に。
fn build_animation_frames(
    runtime: &RuntimePaths,
    asset_server: &AssetServer,
    character_name: &str,
    animation: &Animation,
    sprite_groups: &std::collections::HashMap<u32, SpriteGroup>,
) -> Option<Vec<FrameRender>> {
    use std::time::Duration;
    let mut out = Vec::with_capacity(animation.frames.len());

    for frame in &animation.frames {
        let Some(layer) = frame.layers.first() else {
            tracing::warn!(frame = frame.index, "battle: frame has no layers, skipping");
            continue;
        };
        let Some(group) = sprite_groups.get(&layer.sprite_group_number) else {
            tracing::warn!(
                frame = frame.index,
                group_number = layer.sprite_group_number,
                "battle: sprite_group_number not found, skipping frame",
            );
            continue;
        };
        let Some(sprite) = find_sprite(group, layer.sprite_index) else {
            tracing::warn!(
                frame = frame.index,
                group = %group.name,
                sprite_index = layer.sprite_index,
                "battle: sprite_index not found, skipping frame",
            );
            continue;
        };
        let abs_path = runtime.sprite_file(character_name, &group.name, &sprite.path);
        let Ok(dims) = png_header::read_png_dimensions(&abs_path) else {
            tracing::warn!(path = %abs_path.display(), "battle: PNG header read failed, skipping frame");
            continue;
        };
        let (fx, fy) = frame.pivot_offset_xy();
        let (lx, ly) = layer.pivot_offset_xy();
        let pivot_x = sprite.pivot_point[0] + fx + lx;
        let pivot_y = sprite.pivot_point[1] + fy + ly;
        let anchor = AnimationFrames::anchor_from_pivot(dims[0], dims[1], pivot_x, pivot_y);
        let asset_rel = format!(
            "characters/{character_name}/sprite-groups/{}/sprites/{}",
            group.name, sprite.path
        );
        out.push(FrameRender {
            handle: asset_server.load(&asset_rel),
            anchor,
            duration: Duration::from_millis(u64::from(frame.duration)),
            flip_x: flip_x_of(frame.flip) ^ flip_x_of(layer.flip),
            alpha: layer.transparency.clamp(0.0, 1.0),
        });
    }

    if out.is_empty() {
        tracing::warn!(animation = %animation.name, "battle: animation produced no renderable frames");
        return None;
    }
    Some(out)
}

fn find_sprite(group: &SpriteGroup, sprite_index: u32) -> Option<&SpriteEntry> {
    group.sprites.iter().find(|s| s.index == sprite_index)
}
