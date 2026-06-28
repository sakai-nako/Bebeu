//! Battle scene。Project の最初の player と Level の背景を描画する。
//!
//! 座標系は ADR-0023 に従い、`shared::projection` 経由で Bevy world に変換する。
//! player は Character::load_directory で sprite-groups と animations を populate し、
//! role=Walk の Animation を選んで `AnimationFrames` を構築する (Phase 3: layer は
//! 各 frame の先頭 1 つだけを使う)。
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::app::{PixelPerfectTarget, SceneState};
use crate::entities::character::{
    AiConfig, Animation, AttackBox, AttackBoxMeta, AttackBoxOverride, Character, Frame, HitBox,
    Role, SpriteEntry, SpriteGroup,
};
use crate::entities::level::{Level, OpponentTrigger};
use crate::entities::project::Project;
use crate::features::character::{
    AiCommand, AllyBrain, AnimationData, AnimationFrames, AttackHitConsumed, AttackOutcome,
    BodyBox, BotBrain, CharacterDepth, CharacterState, Combatant, Controller, EnemyAnimationSet,
    EnemyTag, Facing, FrameRender, HitPoints, KinematicVel, LastEngagedWith, MainCamera,
    MeleeBrain, MeleeConfig, PhysicsParams, PlayerAnimationLibrary, PlayerSpriteGroupRegistry,
    Side, SimulationSet, SoundDispatch, VSYNC_TICK, WorldPosition, bake_character_sounds,
};
use crate::shared::PlayerId;
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
        app.add_systems(OnEnter(SceneState::Battle), setup)
            .add_systems(
                Update,
                // Level Resource は battle setup で初めて挿入されるので、title 中に走ると panic する。
                spawn_opponents_on_trigger
                    .run_if(resource_exists::<Level>)
                    .in_set(SimulationSet::Active),
            );
    }
}

#[allow(clippy::too_many_lines)] // Bevy scene setup は spawn のチェーンで長くなりがち
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    runtime: Res<RuntimePaths>,
    project: Option<Res<Project>>,
    pixel_perfect: Option<Res<PixelPerfectTarget>>,
) {
    tracing::info!("battle: enter");

    // Camera 描画先: pixel_perfect の中間 texture (ADR-0026)。Resource 不在 (smoke
    // test 等) のときは default (= primary window) へ直接描画する。
    // `RenderTarget` は Camera の require component なので tuple に並べて spawn する。
    let camera_target = pixel_perfect.as_ref().map_or(RenderTarget::default(), |t| {
        RenderTarget::Image(t.image.clone().into())
    });

    let Some(project) = project else {
        tracing::warn!("battle: no Project resource — running with engine defaults");
        commands.spawn((Camera2d, camera_target));
        return;
    };

    // Level
    let level = project
        .levels
        .first()
        .and_then(
            |name| match Level::load_from_file(&runtime.level_file(name), name) {
                Ok(l) => Some(l),
                Err(err) => {
                    tracing::warn!(error = %err, level = %name, "battle: level load failed");
                    None
                }
            },
        )
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
        camera_target,
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
            // `{err:#}` で anyhow の context chain (parse 失敗 → serde の field 詳細など)
            // を 1 行に連結して出す。`%err` 単独だと最上位の context しか出ず原因不明になる。
            tracing::warn!(error = %format!("{err:#}"), "battle: character load failed");
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

    // `Role::all_loadable()` の全 Role について Animation を build_animation_frames して
    // library に投入 (Custom は除外)。起動時に PNG header を全部読むので、以降の state
    // 切替は O(1)。未登録 Role は state_machine 側の `resolve_animation_role` (ADR-0025
    // 4 段 fallback) が劣化させるので破綻しない。
    let mut library = PlayerAnimationLibrary::default();
    for &role in Role::all_loadable() {
        let Some(anim) = character.animations.iter().find(|a| a.role == role) else {
            // 基本系 (Idle/Walk/Attack) の不在は warn、Knockback 系は debug (fallback が効く)。
            if matches!(role, Role::Idle | Role::Walk | Role::Attack) {
                tracing::warn!(?role, "battle: character has no animation for role");
            } else {
                tracing::debug!(
                    ?role,
                    "battle: character has no animation for role (will fall back)",
                );
            }
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

    // 初期表示は Idle で spawn。state_machine::sync_animation が CharacterState 変化を
    // 検知して以降の hot swap を担当する。
    let Some(initial) = library.get(Role::Idle).cloned() else {
        tracing::warn!("battle: cannot spawn player without idle animation");
        commands.insert_resource(library);
        return;
    };

    let player_translation = projection::world_to_bevy_f32(
        level.player_spawn_x as f32,
        0.0,
        level.player_spawn_z as f32,
    );
    let first_handle = initial.frames[0].handle.clone();
    let first_anchor = initial.frames[0].anchor;
    tracing::info!(
        translation = ?player_translation,
        "battle: spawning player (initial state = Idle)",
    );
    let player_pos = WorldPosition::new(
        level.player_spawn_x as f32,
        0.0,
        level.player_spawn_z as f32,
    );
    let player_entity = commands
        .spawn((
            Sprite::from_image(first_handle),
            first_anchor,
            Transform::from_translation(player_translation),
            AnimationFrames::new(initial.frames, initial.is_loop, initial.loop_start_index),
            // ADR-0038: Hero side + Human controller = 旧 Player marker と等価。
            // bundle tuple 15 上限のため Side のみここで入れ、Controller / PlayerId は
            // spawn 後 insert に流す (= 既存 LastEngagedWith / AiCommand と同方針)。
            // Phase 1 (ADR-0030) の P1 only MVP は維持。
            Side::Hero,
            CharacterState::default(),
            Facing::default(),
            player_pos,
            // 初期値は default で良い。`sync_body_box` が次 frame で current_body_boxes 由来の
            // box に書き換える。Enemy 側と同じ流れ。これが無いと hitbox debug overlay にも
            // 出ない (draw_hitboxes が BodyBox component を query するため)。
            BodyBox::default_for_world(player_pos),
            AttackHitConsumed::default(),
            CharacterDepth(character.depth),
            // 吹っ飛びフロー用 (ADR-0024 Phase A)。Phase A の player は被弾しないが、
            // 将来の Combatant 統一に向けて attach しておく。
            Combatant::new(&character.physics),
            KinematicVel::default(),
            PhysicsParams(character.physics.clone()),
            // Player も Enemy と同様に HP を持たせる (debug overlay の HP 表示 / 将来の被弾対応)。
            HitPoints::new(character.hp),
        ))
        .id();
    // ADR-0031: engagement tracking。初期は誰とも engaged していない。
    // ADR-0035: AiCommand は PlayerInputController が毎 frame 上書きするので Default で
    // 仮入れしておく。bundle tuple の 15 上限を越えるので spawn 後に insert する。
    // ADR-0038: Controller::Human / PlayerId は Side::Hero と同 entity に attach (spawn
    // 後 insert で 15 上限を回避)。
    commands.entity(player_entity).insert((
        LastEngagedWith::default(),
        AiCommand::default(),
        Controller::Human,
        PlayerId::P1,
    ));
    // ADR-0019: Frame.sound dispatch 用の baked SoundGroup と pending スロット。
    // ADR-0034 / ADR-0036: AttackOutcome は attacker 側 SE 出し分け用に attacker-agnostic に
    // attach (Player にも Enemy にも入れる)。
    commands.entity(player_entity).insert((
        bake_character_sounds(&asset_server, player_name, &character),
        SoundDispatch::default(),
        AttackOutcome::default(),
    ));

    // ADR-0035 Phase 3 / ADR-0038 Phase 4: Player 自動化の発火経路は 2 つ:
    //
    // 1. env var `BEATEMUP_PLAYER_BOT` 非空 (Phase 3 既存) — sample-projects との相性が良い
    //    切替方法。BEATEMUP_PROJECT / BEATEMUP_RUNTIME_DIR と同じ「非空で有効」規約。
    // 2. hero character YAML の `ai: kind: bot` (Phase 4 新規) — character YAML レベルで
    //    Controller を宣言したいとき用。`BotConfig` を持てるので Bot 専用 param の余地あり。
    //
    // **env var 優先 + 両立**: 両方指定された場合 env var 側が勝つ (= 既存挙動の回帰なし、
    // ADR-0035 Phase 3 補追規約)。env が指定された場合は YAML の BotConfig より env 経路を
    // 優先し、`MeleeConfig::default()` を使う (= Phase 3 の挙動を維持)。env なしで YAML が
    // `ai: kind: bot` なら YAML 由来の `BotConfig` を `MeleeConfig` に詰め替えて使う。
    //
    // 排他 (= 手動入力 system との競合解決) は `player_input_controller` 側の
    // `Without<BotBrain>` filter で担保 (Phase 3 案 A)。
    let env_bot = std::env::var("BEATEMUP_PLAYER_BOT")
        .ok()
        .is_some_and(|v| !v.is_empty());
    let yaml_bot_cfg: Option<MeleeConfig> = match &character.ai {
        Some(AiConfig::Bot(cfg)) => Some(cfg.clone().into_melee_config()),
        Some(AiConfig::Melee(_)) => {
            tracing::warn!(
                character = %character.name,
                "battle: hero character has ai: kind: melee — ignored (only `bot` or null is supported on hero)",
            );
            None
        }
        Some(AiConfig::Ally(_)) => {
            tracing::warn!(
                character = %character.name,
                "battle: hero character has ai: kind: ally — ignored (use project.allies entry, not players)",
            );
            None
        }
        None => None,
    };
    if env_bot || yaml_bot_cfg.is_some() {
        // env 指定が優先: env_bot=true なら MeleeConfig::default()、それ以外なら YAML 由来。
        let bot_cfg = if env_bot {
            MeleeConfig::default()
        } else {
            yaml_bot_cfg.unwrap_or_default()
        };
        tracing::info!(
            env = env_bot,
            yaml = matches!(&character.ai, Some(AiConfig::Bot(_))),
            "battle: attaching BotBrain to player",
        );
        // Controller::Ai で旧 Controller::Human を上書き (= 自動化に切り替え)。
        commands
            .entity(player_entity)
            .insert((BotBrain::new(bot_cfg), Controller::Ai));
    }

    // ADR-0035 Phase 2: project.allies に書かれた味方 NPC を Player の隣に spawn する。
    // Ally character は ai: kind: ally を持つ前提で、`AllyBrain` + `AiCommand` を attach する。
    // Player との被弾交差は無く、Enemy → Player の attack 解決にも入らない (= damage 不発)。
    let mut ally_spawn_x_offset = -40.0_f32;
    for ally_name in &project.allies {
        spawn_ally(
            &mut commands,
            &runtime,
            &asset_server,
            ally_name,
            level.player_spawn_x as f32 + ally_spawn_x_offset,
            level.player_spawn_z as f32,
        );
        // 2 体目以降は更に Player 後方にずらす (簡易: 同じ場所に重ねない)。
        ally_spawn_x_offset -= 24.0;
    }

    commands.insert_resource(library);
    // ADR-0033: HUD の player_icon が character の sprite_groups を引けるよう registry に登録。
    // Character::load_directory が返した sprite_groups と name を P1 ぶんだけ持ち越す
    // (Phase 1 の MVP は P1 のみ、ADR-0030)。
    let mut sprite_registry = PlayerSpriteGroupRegistry::default();
    sprite_registry.insert(
        PlayerId::P1,
        character.name.clone(),
        character.sprite_groups.clone(),
    );
    commands.insert_resource(sprite_registry);
    // ai::apply_command が areas を読めるよう Resource として注入する (ADR-0022 fail-soft)。
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
            // Frame.ticks (60Hz tick 数) を Duration に変換。tick = 1 / 60 秒 = VSYNC_TICK。
            // 0 tick は「engine 既定」相当として最低 1 tick に clamp する (= 1 vsync 表示)。
            duration: VSYNC_TICK * frame.ticks.max(1),
            flip_x: flip_x_of(frame.flip) ^ flip_x_of(layer.flip),
            alpha: layer.transparency.clamp(0.0, 1.0),
            attack_meta: extract_attack_meta(frame, sprite),
            attack_box_geom: extract_attack_box_geom(frame, sprite),
            body_box_geoms: extract_body_box_geoms(frame, sprite),
            body_box_disabled: extract_body_box_disabled(frame),
            sprite_pivot: [pivot_x, pivot_y],
            image_dims: dims,
            frame_sound: frame.sound,
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

/// Frame の attack_box_overrides を editor 互換の 3-state で解釈し、最終的に有効な
/// AttackBox (先頭要素) を sprite と merge して返す。優先順は:
/// - `frame.attack_box_overrides == None` (Inherit) → `sprite.attack_boxes` の先頭を clone
/// - `frame.attack_box_overrides == Some(empty)` (Disable) → `None`
/// - `frame.attack_box_overrides == Some(non-empty)` (Override) → override の先頭を
///   sprite の先頭と field 単位で merge: override.hitbox が `None` なら sprite から継承、
///   override.meta が `None` なら sprite から継承。両方の継承元も無く override も `None`
///   の場合は (= hitbox が決定できないので) `None` を返す。
///
/// engine が使うのは現状 **先頭要素だけ** で、複数 box の合成は未実装。
#[must_use]
fn resolve_attack_box(frame: &Frame, sprite: &SpriteEntry) -> Option<AttackBox> {
    let sprite_first = sprite
        .attack_boxes
        .as_deref()
        .and_then(<[AttackBox]>::first);
    match frame.attack_box_overrides.as_deref() {
        None => sprite_first.cloned(),
        Some([]) => None,
        Some(overrides) => {
            let ov = overrides.first()?;
            let hitbox = ov
                .hitbox
                .clone()
                .or_else(|| sprite_first.map(|s| s.hitbox.clone()))?;
            let meta = ov.meta.or_else(|| sprite_first.and_then(|s| s.meta));
            Some(AttackBox { hitbox, meta })
        }
    }
}

/// `resolve_attack_box` の `meta` を返す。`None` のときは攻撃判定なし (= meta が
/// sprite / override どちらにも書かれていない、または override で Disable された frame)。
/// FrameRender はこの値を持って attack 系 system に流す (damage / knockback / hit_stop
/// は呼び出し側がここから派生して読む)。
#[must_use]
fn extract_attack_meta(frame: &Frame, sprite: &SpriteEntry) -> Option<AttackBoxMeta> {
    resolve_attack_box(frame, sprite).and_then(|ab| ab.meta)
}

/// `resolve_attack_box` の `hitbox`。`None` で attack 判定なし。
/// 世界座標への変換は `shared::projection::world_box_from_hitbox` が sprite_pivot /
/// char_pos / facing / char_depth を見て行う。
#[must_use]
fn extract_attack_box_geom(frame: &Frame, sprite: &SpriteEntry) -> Option<HitBox> {
    resolve_attack_box(frame, sprite).map(|ab| ab.hitbox)
}

/// Frame の body_box_overrides を editor 互換の 3-state で解釈し、有効な BodyBox 列を返す。
/// `resolve_attack_box` の BodyBox 版。Inherit → sprite.body_boxes、Disable → 空、
/// Override → override の列。
#[must_use]
fn resolve_body_boxes<'a>(frame: &'a Frame, sprite: &'a SpriteEntry) -> &'a [HitBox] {
    match frame.body_box_overrides.as_deref() {
        None => sprite.body_boxes.as_deref().unwrap_or(&[]),
        Some(v) => v,
    }
}

/// `resolve_body_boxes` の結果を `Vec<HitBox>` に clone。FrameRender に焼き込む形。
#[must_use]
fn extract_body_box_geoms(frame: &Frame, sprite: &SpriteEntry) -> Vec<HitBox> {
    resolve_body_boxes(frame, sprite).to_vec()
}

/// ADR-0024: frame が **明示的に** BodyBox を Disable しているか
/// (= `body_box_overrides: []`)。`true` のとき `BodyBox.disabled` が立ち、`aabb_intersects`
/// が無条件で false (= 無敵 frame) を返す。
///
/// `null` override (Inherit) や Some(non-empty) (Override) では false。
/// sprite に body_boxes が無くて override も `null` で結果が empty になるケース (= 安全網
/// fallback) も false (= 通常 hittable, default_for_world で持つ)。
#[must_use]
fn extract_body_box_disabled(frame: &Frame) -> bool {
    matches!(frame.body_box_overrides.as_deref(), Some([]))
}

/// Player の world X が `trigger_x` 以上になった最初の trigger を返す。
/// 該当が無ければ `None`。triggers は前方一致順に評価 (=YAML 記述順)。
#[must_use]
pub fn next_triggered_index(triggers: &[OpponentTrigger], player_x: f32) -> Option<usize> {
    triggers.iter().position(|t| player_x >= t.trigger_x as f32)
}

/// 毎 frame、player の X が trigger_x に到達した最初の `OpponentTrigger` を 1 件だけ
/// 消費して enemy を spawn する。発火済みは `Level.opponent_triggers` から remove する
/// ことで「1-shot」を表現する。Enemy は idle Animation で立たせ、左 (player 方向) を向ける。
#[allow(clippy::too_many_lines)]
fn spawn_opponents_on_trigger(
    mut commands: Commands,
    mut level: ResMut<Level>,
    runtime: Res<RuntimePaths>,
    asset_server: Res<AssetServer>,
    // ADR-0038: 旧 `With<Player>` filter は Hero+Human で再表現。Phase 1 (ADR-0030) の
    // P1 only MVP のままなので最初の 1 体を取る。
    player_query: Query<(&WorldPosition, &Side, &Controller)>,
) {
    let Some(player_pos) = player_query.iter().find_map(|(pos, side, ctrl)| {
        (matches!(side, Side::Hero) && matches!(ctrl, Controller::Human)).then_some(pos)
    }) else {
        return;
    };
    let Some(idx) = next_triggered_index(&level.opponent_triggers, player_pos.x) else {
        return;
    };
    let trigger = level.opponent_triggers.remove(idx);
    let character = match Character::load_directory(&runtime, &trigger.character_name) {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(
                error = %err,
                character = %trigger.character_name,
                "battle: opponent load failed, trigger consumed",
            );
            return;
        }
    };
    // idle が無いと立ち絵が作れないので必須。hit は最初は無くてもよい (未登録なら hit 中の
    // 描画は warn だけ吐いて素のまま続く)。
    let Some(idle_data) = build_role_animation_data(
        &runtime,
        &asset_server,
        &trigger.character_name,
        &character,
        Role::Idle,
    ) else {
        tracing::warn!(
            character = %character.name,
            "battle: opponent has no idle animation, skipping spawn",
        );
        return;
    };
    let mut anim_set = EnemyAnimationSet::default();
    anim_set.insert(Role::Idle, idle_data.clone());
    // Idle 以外の全 loadable Role を試行 (Custom 除外、ADR-0025 chain で fallback)。
    for &role in Role::all_loadable() {
        if role == Role::Idle {
            continue; // 既に投入済み
        }
        if let Some(data) = build_role_animation_data(
            &runtime,
            &asset_server,
            &trigger.character_name,
            &character,
            role,
        ) {
            anim_set.insert(role, data);
        }
    }
    let first_handle = idle_data.frames[0].handle.clone();
    let first_anchor = idle_data.frames[0].anchor;
    let translation = projection::world_to_bevy_f32(
        trigger.spawn_x as f32,
        trigger.spawn_y as f32,
        trigger.spawn_z as f32,
    );
    tracing::info!(
        character = %character.name,
        translation = ?translation,
        "battle: spawning opponent",
    );
    let enemy_pos = WorldPosition::new(
        trigger.spawn_x as f32,
        trigger.spawn_y as f32,
        trigger.spawn_z as f32,
    );
    let enemy_entity = commands
        .spawn((
            Sprite::from_image(first_handle),
            first_anchor,
            Transform::from_translation(translation),
            AnimationFrames::new(
                idle_data.frames,
                idle_data.is_loop,
                idle_data.loop_start_index,
            ),
            // ADR-0038: 旧 Enemy marker = Villain side + Ai controller。
            // bundle tuple 15 上限のため Controller は spawn 後 insert に流す
            // (= EnemyTag / AttackOutcome 系と同方針)。
            Side::Villain,
            CharacterState::default(),
            Facing::Left,
            enemy_pos,
            BodyBox::default_for_world(enemy_pos),
            HitPoints::new(character.hp),
            CharacterDepth(character.depth),
            anim_set,
            // 吹っ飛びフロー用 (ADR-0024 Phase A)。被弾側として gauge / 速度 / physics を持つ。
            Combatant::new(&character.physics),
            KinematicVel::default(),
            PhysicsParams(character.physics.clone()),
        ))
        .id();
    // ADR-0031: tag があれば HUD target { tag: ... } から参照できるよう component を attach。
    if let Some(tag) = &character.tag {
        commands.entity(enemy_entity).insert(EnemyTag(tag.clone()));
    }
    // ADR-0035 Phase 1.2: character YAML の `ai:` セクションから Brain を attach。
    // `ai: null` (= AiConfig 未設定) なら Brain なし (= 床に立って何もしない object) として扱い、
    // AiCommand も attach しない。Phase 2 で増えた `Ally` variant は Enemy spawn 経路では
    // 受け付けず warn を吐く (= Ally は project.allies 経由で `spawn_ally` から spawn される)。
    if let Some(ai) = &character.ai {
        match ai {
            AiConfig::Melee(cfg) => {
                commands
                    .entity(enemy_entity)
                    .insert((MeleeBrain::new(cfg.clone()), AiCommand::default()));
            }
            AiConfig::Ally(_) => {
                tracing::warn!(
                    character = %character.name,
                    "battle: enemy character has ai: kind: ally — ignoring Brain attach \
                     (allies should be declared in project.allies, not opponents)",
                );
            }
            // ADR-0038: `Bot` は hero (Player spawn 経路) でだけ意味を持つ。Villain には
            // 適用しない (= 単に warn して MeleeBrain も attach しない)。
            AiConfig::Bot(_) => {
                tracing::warn!(
                    character = %character.name,
                    "battle: enemy character has ai: kind: bot — ignoring Brain attach \
                     (bot is hero-only; use ai: kind: melee for villain)",
                );
            }
        }
    }
    // ADR-0019: Frame.sound dispatch 用の baked SoundGroup と pending スロット。
    // ADR-0036: AttackHitConsumed と AttackOutcome を attacker-agnostic に attach。
    // Enemy → Player の damage / SE 出し分けを Player → Enemy と対称に動かす。spawn の
    // 15-tuple 上限に収まらないのでここで insert する。
    // ADR-0038: Controller::Ai は Side::Villain と同 entity に attach (spawn 後 insert)。
    commands.entity(enemy_entity).insert((
        bake_character_sounds(&asset_server, &trigger.character_name, &character),
        SoundDispatch::default(),
        AttackHitConsumed::default(),
        AttackOutcome::default(),
        Controller::Ai,
    ));
}

/// ADR-0035 Phase 2: project.allies に書かれた味方 NPC を spawn する。spawn 構造は
/// `spawn_opponents_on_trigger` の Enemy 経路と概ね同じ (per-entity `EnemyAnimationSet` を
/// 使い、`sync_enemy_animation` に animation 切替を任せる) が、Brain は `AllyBrain` 固定で、
/// `Ally` marker (= Enemy / Player と排他) を attach する。
/// `character.ai` が `Ally` 以外 (Melee or 未指定) のときは warn を出して spawn を諦める
/// (= ally として宣言された character は ai: kind: ally を持つべき)。
fn spawn_ally(
    commands: &mut Commands,
    runtime: &RuntimePaths,
    asset_server: &AssetServer,
    character_name: &str,
    spawn_x: f32,
    spawn_z: f32,
) {
    let character = match Character::load_directory(runtime, character_name) {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(
                error = %format!("{err:#}"),
                character = %character_name,
                "battle: ally load failed",
            );
            return;
        }
    };
    let Some(AiConfig::Ally(cfg)) = character.ai.clone() else {
        tracing::warn!(
            character = %character.name,
            "battle: ally character has no ai: kind: ally; skipping spawn",
        );
        return;
    };
    let Some(idle_data) = build_role_animation_data(
        runtime,
        asset_server,
        character_name,
        &character,
        Role::Idle,
    ) else {
        tracing::warn!(
            character = %character.name,
            "battle: ally has no idle animation, skipping spawn",
        );
        return;
    };
    let mut anim_set = EnemyAnimationSet::default();
    anim_set.insert(Role::Idle, idle_data.clone());
    for &role in Role::all_loadable() {
        if role == Role::Idle {
            continue;
        }
        if let Some(data) =
            build_role_animation_data(runtime, asset_server, character_name, &character, role)
        {
            anim_set.insert(role, data);
        }
    }
    let first_handle = idle_data.frames[0].handle.clone();
    let first_anchor = idle_data.frames[0].anchor;
    let translation = projection::world_to_bevy_f32(spawn_x, 0.0, spawn_z);
    tracing::info!(
        character = %character.name,
        translation = ?translation,
        "battle: spawning ally",
    );
    let ally_pos = WorldPosition::new(spawn_x, 0.0, spawn_z);
    let ally_entity = commands
        .spawn((
            Sprite::from_image(first_handle),
            first_anchor,
            Transform::from_translation(translation),
            AnimationFrames::new(
                idle_data.frames,
                idle_data.is_loop,
                idle_data.loop_start_index,
            ),
            // ADR-0038: 旧 Ally marker = Hero side + Ai controller。
            // bundle tuple 15 上限のため Controller は spawn 後 insert に流す。
            Side::Hero,
            CharacterState::default(),
            Facing::Right,
            ally_pos,
            BodyBox::default_for_world(ally_pos),
            HitPoints::new(character.hp),
            CharacterDepth(character.depth),
            anim_set,
            Combatant::new(&character.physics),
            KinematicVel::default(),
            PhysicsParams(character.physics.clone()),
        ))
        .id();
    commands.entity(ally_entity).insert((
        AllyBrain::new(cfg),
        AiCommand::default(),
        Controller::Ai,
    ));
    // ADR-0038 Phase 4: Ally の attack は ally→enemy 経路ですでに damage を出すが、加えて
    // Enemy→Ally も resolve_villain_attacks (= 旧 resolve_enemy_attacks) で Hero side
    // victim として通る (= ally が enemy で knockback / KO される)。
    // attacker 側 SE 出し分け用に AttackOutcome / AttackHitConsumed / SoundDispatch /
    // baked SoundGroup は引き続き対称に attach (Enemy / Player と同じ作法、ADR-0019 /
    // ADR-0034 / ADR-0036)。
    commands.entity(ally_entity).insert((
        bake_character_sounds(asset_server, character_name, &character),
        SoundDispatch::default(),
        AttackHitConsumed::default(),
        AttackOutcome::default(),
    ));
}

/// `character` の `role` Animation を引いて `build_animation_frames` で frame 列を作り、
/// `AnimationData` として返す。role が存在しない or frame ビルド失敗で `None`。
fn build_role_animation_data(
    runtime: &RuntimePaths,
    asset_server: &AssetServer,
    character_name: &str,
    character: &Character,
    role: Role,
) -> Option<AnimationData> {
    let anim = character.animations.iter().find(|a| a.role == role)?;
    let frames = build_animation_frames(
        runtime,
        asset_server,
        character_name,
        anim,
        &character.sprite_groups,
    )?;
    Some(AnimationData {
        frames,
        is_loop: anim.is_loop,
        loop_start_index: anim.loop_start_index as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trig(trigger_x: i32) -> OpponentTrigger {
        OpponentTrigger {
            character_name: "enemy".into(),
            trigger_x,
            spawn_x: 0,
            spawn_y: 0,
            spawn_z: 0,
        }
    }

    #[test]
    fn next_triggered_index_returns_none_when_player_behind_all() {
        let triggers = vec![trig(100), trig(200)];
        assert!(next_triggered_index(&triggers, 50.0).is_none());
    }

    #[test]
    fn next_triggered_index_returns_first_in_order() {
        // YAML 順を尊重する: 100 と 200 の両方を超えていても先頭 (idx 0) から消費。
        let triggers = vec![trig(100), trig(200)];
        assert_eq!(next_triggered_index(&triggers, 300.0), Some(0));
    }

    #[test]
    fn next_triggered_index_skips_unreached_until_threshold() {
        // 先頭が未到達、2 番目を到達。順序を保ったまま 2 番目を返す。
        let triggers = vec![trig(500), trig(150)];
        assert_eq!(next_triggered_index(&triggers, 200.0), Some(1));
    }

    #[test]
    fn next_triggered_index_inclusive_on_boundary() {
        let triggers = vec![trig(200)];
        assert_eq!(next_triggered_index(&triggers, 200.0), Some(0));
    }

    fn ab(damage: u32) -> AttackBox {
        AttackBox {
            hitbox: HitBox {
                top_left: [0, 0],
                bottom_right: [10, 10],
                depth: None,
            },
            meta: Some(AttackBoxMeta {
                damage,
                ..AttackBoxMeta::default()
            }),
        }
    }

    /// 両方 Some の override (= sprite を完全上書き)。
    fn ab_override(damage: u32) -> AttackBoxOverride {
        AttackBoxOverride {
            hitbox: Some(HitBox {
                top_left: [0, 0],
                bottom_right: [10, 10],
                depth: None,
            }),
            meta: Some(AttackBoxMeta {
                damage,
                ..AttackBoxMeta::default()
            }),
        }
    }

    fn sprite_with(boxes: Option<Vec<AttackBox>>) -> SpriteEntry {
        SpriteEntry {
            attack_boxes: boxes,
            ..SpriteEntry::default()
        }
    }

    fn meta_damage(frame: &Frame, sprite: &SpriteEntry) -> Option<u32> {
        extract_attack_meta(frame, sprite).map(|m| m.damage)
    }

    #[test]
    fn extract_attack_meta_inherits_from_sprite_when_no_override() {
        // frame.overrides=None → sprite.attack_boxes が使われる
        let frame = Frame::default();
        let sprite = sprite_with(Some(vec![ab(30)]));
        assert_eq!(meta_damage(&frame, &sprite), Some(30));
    }

    #[test]
    fn extract_attack_meta_disabled_when_override_empty_vec() {
        // frame.overrides=Some(empty) → Disable (sprite を見ない)
        let frame = Frame {
            attack_box_overrides: Some(vec![]),
            ..Frame::default()
        };
        let sprite = sprite_with(Some(vec![ab(30)]));
        assert!(extract_attack_meta(&frame, &sprite).is_none());
    }

    #[test]
    fn extract_attack_meta_overrides_sprite_when_non_empty() {
        // frame.overrides=Some(non-empty) → override の値 (sprite は見ない)
        let frame = Frame {
            attack_box_overrides: Some(vec![ab_override(40)]),
            ..Frame::default()
        };
        let sprite = sprite_with(Some(vec![ab(30)]));
        assert_eq!(meta_damage(&frame, &sprite), Some(40));
    }

    #[test]
    fn extract_attack_meta_none_when_neither_sprite_nor_override() {
        let frame = Frame::default();
        let sprite = sprite_with(None);
        assert!(extract_attack_meta(&frame, &sprite).is_none());
    }

    #[test]
    fn extract_attack_box_geom_inherits_from_sprite() {
        let frame = Frame::default();
        let sprite = sprite_with(Some(vec![ab(30)]));
        let geom = extract_attack_box_geom(&frame, &sprite).expect("inherited");
        assert_eq!(geom.bottom_right, [10, 10]);
    }

    #[test]
    fn partial_override_meta_only_inherits_hitbox_from_sprite() {
        // override に meta のみ書いた場合、hitbox は sprite から継承される。
        let frame = Frame {
            attack_box_overrides: Some(vec![AttackBoxOverride {
                hitbox: None,
                meta: Some(AttackBoxMeta {
                    damage: 99,
                    ..AttackBoxMeta::default()
                }),
            }]),
            ..Frame::default()
        };
        let sprite = sprite_with(Some(vec![ab(30)]));
        // meta は override 値が勝つ
        assert_eq!(meta_damage(&frame, &sprite), Some(99));
        // hitbox は sprite から継承される (bottom_right = [10, 10])
        let geom = extract_attack_box_geom(&frame, &sprite).expect("hitbox should be inherited");
        assert_eq!(geom.bottom_right, [10, 10]);
    }

    #[test]
    fn partial_override_hitbox_only_inherits_meta_from_sprite() {
        // override に hitbox のみ書いた場合、meta は sprite から継承される。
        let alt = HitBox {
            top_left: [50, 50],
            bottom_right: [80, 80],
            depth: None,
        };
        let frame = Frame {
            attack_box_overrides: Some(vec![AttackBoxOverride {
                hitbox: Some(alt.clone()),
                meta: None,
            }]),
            ..Frame::default()
        };
        let sprite = sprite_with(Some(vec![ab(30)]));
        // hitbox は override 値が勝つ
        let geom = extract_attack_box_geom(&frame, &sprite).expect("hitbox should be overridden");
        assert_eq!(geom.bottom_right, alt.bottom_right);
        // meta は sprite から継承される (damage = 30)
        assert_eq!(meta_damage(&frame, &sprite), Some(30));
    }

    #[test]
    fn partial_override_both_none_inherits_everything_from_sprite() {
        // override の両 field が None なら sprite を完全継承 (= override しない場合と同じ)。
        let frame = Frame {
            attack_box_overrides: Some(vec![AttackBoxOverride::default()]),
            ..Frame::default()
        };
        let sprite = sprite_with(Some(vec![ab(30)]));
        assert_eq!(meta_damage(&frame, &sprite), Some(30));
        let geom = extract_attack_box_geom(&frame, &sprite).expect("inherited");
        assert_eq!(geom.bottom_right, [10, 10]);
    }

    #[test]
    fn partial_override_hitbox_only_without_sprite_yields_no_meta() {
        // override に hitbox のみで sprite 側 attack_boxes も無いとき、hitbox は使われるが
        // meta は None になる (継承元が無い)。damage 計算は None になる。
        let alt = HitBox {
            top_left: [0, 0],
            bottom_right: [5, 5],
            depth: None,
        };
        let frame = Frame {
            attack_box_overrides: Some(vec![AttackBoxOverride {
                hitbox: Some(alt),
                meta: None,
            }]),
            ..Frame::default()
        };
        let sprite = sprite_with(None);
        assert!(meta_damage(&frame, &sprite).is_none());
        assert!(extract_attack_box_geom(&frame, &sprite).is_some());
    }

    #[test]
    fn partial_override_meta_only_without_sprite_yields_no_box() {
        // override に meta のみで sprite 側に attack_boxes 無し → hitbox の継承元が無く None。
        let frame = Frame {
            attack_box_overrides: Some(vec![AttackBoxOverride {
                hitbox: None,
                meta: Some(AttackBoxMeta {
                    damage: 50,
                    ..AttackBoxMeta::default()
                }),
            }]),
            ..Frame::default()
        };
        let sprite = sprite_with(None);
        assert!(extract_attack_box_geom(&frame, &sprite).is_none());
        assert!(meta_damage(&frame, &sprite).is_none());
    }

    fn body_hb() -> HitBox {
        HitBox {
            top_left: [14, 18],
            bottom_right: [34, 60],
            depth: Some(16),
        }
    }

    fn sprite_with_body(boxes: Option<Vec<HitBox>>) -> SpriteEntry {
        SpriteEntry {
            body_boxes: boxes,
            ..SpriteEntry::default()
        }
    }

    #[test]
    fn extract_body_box_geoms_inherits_from_sprite_when_no_override() {
        let frame = Frame::default();
        let sprite = sprite_with_body(Some(vec![body_hb()]));
        assert_eq!(extract_body_box_geoms(&frame, &sprite), vec![body_hb()]);
    }

    #[test]
    fn extract_body_box_geoms_disabled_when_override_empty_vec() {
        let frame = Frame {
            body_box_overrides: Some(vec![]),
            ..Frame::default()
        };
        let sprite = sprite_with_body(Some(vec![body_hb()]));
        assert!(extract_body_box_geoms(&frame, &sprite).is_empty());
    }

    #[test]
    fn extract_body_box_geoms_overrides_sprite_when_non_empty() {
        let alt = HitBox {
            top_left: [0, 0],
            bottom_right: [5, 5],
            depth: None,
        };
        let frame = Frame {
            body_box_overrides: Some(vec![alt.clone()]),
            ..Frame::default()
        };
        let sprite = sprite_with_body(Some(vec![body_hb()]));
        assert_eq!(extract_body_box_geoms(&frame, &sprite), vec![alt]);
    }

    #[test]
    fn extract_body_box_geoms_empty_when_neither_sprite_nor_override() {
        let frame = Frame::default();
        let sprite = sprite_with_body(None);
        assert!(extract_body_box_geoms(&frame, &sprite).is_empty());
    }

    #[test]
    fn extract_body_box_disabled_only_for_explicit_empty_override() {
        // Override Some([]) = ADR-0024 Disable (= 無敵 frame)。
        let f_empty = Frame {
            body_box_overrides: Some(vec![]),
            ..Frame::default()
        };
        assert!(extract_body_box_disabled(&f_empty));

        // null override = Inherit (= sprite に従う)。Disable ではない。
        let f_inherit = Frame::default();
        assert!(!extract_body_box_disabled(&f_inherit));

        // Some(non-empty) = 通常の Override。Disable ではない。
        let f_override = Frame {
            body_box_overrides: Some(vec![body_hb()]),
            ..Frame::default()
        };
        assert!(!extract_body_box_disabled(&f_override));
    }
}
