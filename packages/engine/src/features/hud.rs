//! Gameplay HUD slice (ADR-0029, ADR-0031)。
//!
//! `Project.hud.elements` を読み、battle scene 入場時に Sprite として
//! HUD 要素を spawn する。MainCamera の子として attach することで camera の X 追従に
//! 同期させ、別 system は要らない。
//!
//! 要素の種類: `PlayerHpBar` (矩形) / `PlayerHpRing` (annular sector) / `EnemyHpBar`
//! (矩形、target を動的解決)。要素を増やすときは [`HudElement`] に variant と対応する
//! Config struct を生やし、spawn_hud の match と更新 system を 1 ペア追加する。
//!
//! ADR-0031: `anchor_to` が `Some` の要素は他要素の root の child として spawn し、
//! 親が camera 追従していれば相対位置で勝手についていく。`EnemyHpBar` は target を
//! 毎 frame 再解決し、target 不在のときは Visibility::Hidden で隠す。
use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::app::SceneState;
use crate::entities::character::Role;
use crate::entities::project::{
    EnemyHpBarConfig, EnemyOverheadHpBarConfig, EnemyTarget, FillDirection, GaugeStep, HudAnchor,
    HudElement, HudElementAnchor, HudOffset, HudSize, IconShakeParams, OverheadVerticalAnchor,
    PlayerHpBarConfig, PlayerHpRingConfig, PlayerIconConfig, Project, RingDirection,
};
use crate::features::character::{
    AnimationFrames, CharacterState, EnemyTag, HitPoints, HitStopState, LastEngagedWith,
    MainCamera, PlayerSpriteGroupRegistry, Side,
};
use crate::shared::PlayerId;

/// HUD 要素を scene の sprite より手前に描くための z オフセット。
/// 背景が `-1.0`, キャラが 0 付近なので 100 で十分前面に出る。
const HUD_Z: f32 = 100.0;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        // spawn_hud は MainCamera / Player / Project すべてが揃ってから 1 度だけ走る。
        // OnEnter(Battle) だと並列実行で MainCamera が間に合わないことがあるため、
        // 「HudRoot が無いとき毎 frame 条件 check」する idempotent system にする。
        app.add_systems(Update, spawn_hud.run_if(in_state(SceneState::Battle)))
            .add_systems(OnExit(SceneState::Battle), despawn_hud)
            .add_systems(
                Update,
                (
                    update_player_hp_bar,
                    update_player_hp_ring,
                    update_enemy_hp_bar,
                    // ADR-0032: world-anchored overhead bar は新 Enemy の検出 → spawn と、
                    // 既存 bar の更新を分けて回す。spawn は `Added<Enemy>` で 1 度だけ走る。
                    spawn_enemy_overhead_hp_bars,
                    update_enemy_overhead_hp_bar,
                    // ADR-0033: Player の CharacterState → sprite swap、HP 減 / attack hit → 振動。
                    update_player_icon_sprite,
                    detect_icon_damage,
                    detect_icon_attack_hit,
                    tick_icon_shake,
                )
                    .run_if(in_state(SceneState::Battle)),
            );
    }
}

/// HUD 要素 root のマーカー。OnExit(Battle) で despawn する。
#[derive(Component)]
struct HudRoot;

/// 1 本の gauge segment。HP がこの segment の `hp_low..hp_high` 範囲にあるとき
/// `(current - hp_low) / (hp_high - hp_low)` の比率で sprite を縮める。
#[derive(Component)]
struct PlayerHpBarGauge {
    target: PlayerId,
    hp_low: f32,
    hp_high: f32,
    full_size: Vec2,
    fill_direction: FillDirection,
}

/// Ring の 1 segment。HP がこの範囲のとき `full_start_rad..full_end_rad` を ratio で詰める。
/// rad は数学標準 (3 時 = 0°、反時計回り正)。
#[derive(Component)]
struct PlayerHpRingGauge {
    target: PlayerId,
    hp_low: f32,
    hp_high: f32,
    outer_r: f32,
    inner_r: f32,
    full_start_rad: f32,
    full_end_rad: f32,
}

/// EnemyHpBar root に attach する meta (ADR-0031)。target は毎 frame 再解決するため
/// config の target を component に持って引き回す。Phase 2 では gauge_step は
/// **FixedCount(1) 強制** (target 切替時の segment 再計算を避ける) で、cfg の値は
/// `target` を resolve した最初の enemy の max_hp に対する単一 gauge として使う。
#[derive(Component)]
struct EnemyHpBarRoot {
    target: EnemyTarget,
    fill_direction: FillDirection,
    inner_size: Vec2,
}

/// EnemyHpBar の単一 gauge sprite (Phase 2: 1 本のみ)。`full_size` はゲージの最大寸法。
/// 毎 frame `current_hp / max_hp` で sprite の `custom_size` を縮める。
#[derive(Component)]
struct EnemyHpBarGauge {
    full_size: Vec2,
    fill_direction: FillDirection,
}

/// world-anchored な Enemy overhead bar の root (ADR-0032)。
/// 親 Enemy entity の child として attach し、Bevy hierarchy で位置追従する。
/// `enemy` は親 Enemy entity の Entity id (HitPoints と AnimationFrames を引くために保持)。
/// `vertical_anchor` / `offset_y` は update system で毎 frame Y 再計算に使う。
#[derive(Component)]
struct EnemyOverheadHpBarRoot {
    enemy: Entity,
    vertical_anchor: OverheadVerticalAnchor,
    offset_y: f32,
}

/// EnemyOverheadHpBar の gauge sprite (単一)。`update_enemy_overhead_hp_bar` が縮める。
#[derive(Component)]
struct EnemyOverheadHpBarGauge {
    full_size: Vec2,
    fill_direction: FillDirection,
}

#[allow(clippy::too_many_arguments)]
fn spawn_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    project: Option<Res<Project>>,
    sprite_registry: Option<Res<PlayerSpriteGroupRegistry>>,
    camera_query: Query<Entity, With<MainCamera>>,
    player_query: Query<(&PlayerId, &HitPoints)>,
    existing: Query<(), With<HudRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !existing.is_empty() {
        return;
    }
    let Some(project) = project else {
        return;
    };
    let Ok(camera) = camera_query.single() else {
        return;
    };
    // Player 系要素は max HP を knownしたい。Player 不在の場合でも EnemyHpBar (Tag/NthEnemy)
    // は spawn できるべきだが、Phase 2 では Player が居ないなら HUD 全体を skip して良い
    // (smoke test / no-character フローを優先)。
    if player_query.is_empty() {
        return;
    }

    let viewport = (
        project.resolution.width as f32,
        project.resolution.height as f32,
    );

    // ADR-0031: id を持つ要素を後続要素から参照できるよう entity と size を覚える。
    // 参照は前方向 (= YAML 上で親が先に書かれている) のみ。これは serde 順を尊重する
    // と同時に、循環参照を構造的に禁じる効果がある。
    let mut spawned_by_id: HashMap<String, (Entity, HudSize)> = HashMap::new();

    for element in &project.hud.elements {
        // world-anchored variant は per-enemy spawn 経路で扱う (このループでは skip)。
        if !element.is_screen_anchored() {
            continue;
        }
        // 親 entity と root の translation を決める。anchor_to 優先、無ければ screen anchor。
        let parent_and_translation = resolve_parent_and_translation(
            element.anchor_to(),
            element.anchor(),
            element.offset(),
            viewport,
            camera,
            &spawned_by_id,
        );
        let Some((parent_entity, root_translation)) = parent_and_translation else {
            // anchor_to.id が未解決 → 要素 skip (resolve_parent_and_translation 内で warn 済)。
            continue;
        };

        // kind ごとに target を解決して spawn。
        let root_entity = match element {
            HudElement::PlayerHpBar(cfg) => {
                let Some((_, hp)) = player_query.iter().find(|(p, _)| **p == cfg.target) else {
                    tracing::warn!(target = ?cfg.target, "hud: target player not present, skipping element");
                    continue;
                };
                Some(spawn_player_hp_bar(
                    &mut commands,
                    parent_entity,
                    root_translation,
                    cfg,
                    hp.max as f32,
                ))
            }
            HudElement::PlayerHpRing(cfg) => {
                let Some((_, hp)) = player_query.iter().find(|(p, _)| **p == cfg.target) else {
                    tracing::warn!(target = ?cfg.target, "hud: target player not present, skipping element");
                    continue;
                };
                Some(spawn_player_hp_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    parent_entity,
                    root_translation,
                    cfg,
                    hp.max as f32,
                ))
            }
            HudElement::EnemyHpBar(cfg) => Some(spawn_enemy_hp_bar(
                &mut commands,
                parent_entity,
                root_translation,
                cfg,
            )),
            HudElement::PlayerIcon(cfg) => {
                let Some(registry) = sprite_registry.as_ref() else {
                    tracing::warn!(
                        "hud: PlayerSpriteGroupRegistry resource missing, skipping player_icon"
                    );
                    continue;
                };
                let Some(player_groups) = registry.get(cfg.target) else {
                    tracing::warn!(target = ?cfg.target, "hud: no sprite groups registered for player, skipping player_icon");
                    continue;
                };
                spawn_player_icon(
                    &mut commands,
                    &asset_server,
                    parent_entity,
                    root_translation,
                    cfg,
                    player_groups,
                    &player_query,
                )
            }
            HudElement::EnemyOverheadHpBar(_) => {
                // ADR-0032: world-anchored は spawn_enemy_overhead_hp_bars が `Added<Enemy>` 経路で
                // per-enemy attach する。screen-anchor 経路では何もしない。
                None
            }
        };

        // id を持つ要素は spawned_by_id に登録 (重複は警告)。
        if let (Some(id), Some(root)) = (element.id(), root_entity)
            && spawned_by_id
                .insert(id.to_string(), (root, element.size()))
                .is_some()
        {
            tracing::warn!(id, "hud: duplicate id, later element overrides earlier");
        }
    }
}

/// `anchor_to` が Some なら親要素の root を引いて edge + offset を local 座標で返す。
/// なければ screen anchor + offset から MainCamera 子としての world 座標を返す。
/// `anchor_to.id` が `spawned_by_id` に無いときは warn を出して `None` を返す。
fn resolve_parent_and_translation(
    anchor_to: Option<&HudElementAnchor>,
    screen_anchor: HudAnchor,
    offset: HudOffset,
    viewport: (f32, f32),
    camera: Entity,
    spawned_by_id: &HashMap<String, (Entity, HudSize)>,
) -> Option<(Entity, Vec3)> {
    if let Some(at) = anchor_to {
        let Some((parent, parent_size)) = spawned_by_id.get(&at.id) else {
            tracing::warn!(
                id = %at.id,
                "hud: anchor_to.id not found among previously spawned elements, skipping element",
            );
            return None;
        };
        // 親 root の top-left を基準にした 9 隅 local 座標 (Bevy Y 上正)。
        let edge_local = edge_local_pos(*parent_size, at.edge);
        // offset.y は画面感覚 (下が正) なので Bevy 座標に変換時に符号反転する。
        let translation = Vec3::new(edge_local.x + offset.x, edge_local.y - offset.y, 0.0);
        Some((*parent, translation))
    } else {
        let top_left = top_left_of_element(screen_anchor, offset, viewport);
        Some((camera, Vec3::new(top_left.x, top_left.y, HUD_Z)))
    }
}

/// 親 HUD 要素の bbox の 9 隅 (`HudAnchor`) を、TOP_LEFT を原点 (0, 0) とし、Bevy 座標で返す。
/// X は右が正、Y は上が正 (= TOP_LEFT 基準で下方向は負)。
#[must_use]
fn edge_local_pos(size: HudSize, edge: HudAnchor) -> Vec2 {
    let w = size.w;
    let h = size.h;
    let (x, y) = match edge {
        HudAnchor::TopLeft => (0.0, 0.0),
        HudAnchor::Top => (w * 0.5, 0.0),
        HudAnchor::TopRight => (w, 0.0),
        HudAnchor::Left => (0.0, -h * 0.5),
        HudAnchor::Center => (w * 0.5, -h * 0.5),
        HudAnchor::Right => (w, -h * 0.5),
        HudAnchor::BottomLeft => (0.0, -h),
        HudAnchor::Bottom => (w * 0.5, -h),
        HudAnchor::BottomRight => (w, -h),
    };
    Vec2::new(x, y)
}

fn despawn_hud(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn update_player_hp_bar(
    player_query: Query<(&PlayerId, &HitPoints)>,
    mut gauges: Query<(&mut Sprite, &PlayerHpBarGauge)>,
) {
    for (mut sprite, gauge) in &mut gauges {
        let Some((_, hp)) = player_query.iter().find(|(p, _)| **p == gauge.target) else {
            continue;
        };
        let current = hp.current as f32;
        let denom = gauge.hp_high - gauge.hp_low;
        let ratio = if denom <= 0.0 {
            0.0
        } else {
            ((current - gauge.hp_low) / denom).clamp(0.0, 1.0)
        };
        let Some(size) = sprite.custom_size.as_mut() else {
            continue;
        };
        match gauge.fill_direction {
            FillDirection::LeftToRight | FillDirection::RightToLeft => {
                size.x = gauge.full_size.x * ratio;
            }
            FillDirection::TopToBottom | FillDirection::BottomToTop => {
                size.y = gauge.full_size.y * ratio;
            }
        }
    }
}

fn spawn_player_hp_bar(
    commands: &mut Commands,
    parent: Entity,
    root_translation: Vec3,
    cfg: &PlayerHpBarConfig,
    max_hp: f32,
) -> Entity {
    let outer_size = Vec2::new(cfg.size.w, cfg.size.h);
    let frame_t = cfg.frame.thickness.max(0.0);
    let inner_size = Vec2::new(
        (cfg.size.w - 2.0 * frame_t).max(0.0),
        (cfg.size.h - 2.0 * frame_t).max(0.0),
    );

    let root = commands
        .spawn((
            HudRoot,
            Transform::from_translation(root_translation),
            Visibility::default(),
            ChildOf(parent),
        ))
        .id();

    // 枠 (frame.thickness > 0 のとき outer 全面を frame 色で塗り、その上に inner サイズの
    // bg を重ねる)。枠が不透明なら見た目は「枠 + 内側」レイアウトとして自然に見える。
    if frame_t > 0.0 {
        commands.spawn((
            Sprite::from_color(Color::from(cfg.frame.color), outer_size),
            Anchor::TOP_LEFT,
            Transform::from_xyz(0.0, 0.0, 0.0),
            ChildOf(root),
        ));
    }

    // 内側 bg (frame の thickness 分内側にオフセットして描く)。
    commands.spawn((
        Sprite::from_color(Color::from(cfg.bg_color), inner_size),
        Anchor::TOP_LEFT,
        Transform::from_xyz(frame_t, -frame_t, 0.1),
        ChildOf(root),
    ));

    // 各 gauge segment を spawn。
    let segments = gauge_layout(
        cfg.gauge_step,
        cfg.fill_direction,
        cfg.gauge_gap,
        inner_size,
        max_hp,
    );
    for seg in segments {
        let pos = Vec3::new(seg.origin.x + frame_t, seg.origin.y - frame_t, 0.2);
        commands.spawn((
            Sprite::from_color(Color::from(cfg.fg_color), seg.full_size),
            seg.anchor,
            Transform::from_translation(pos),
            PlayerHpBarGauge {
                target: cfg.target,
                hp_low: seg.hp_low,
                hp_high: seg.hp_high,
                full_size: seg.full_size,
                fill_direction: cfg.fill_direction,
            },
            ChildOf(root),
        ));
    }
    root
}

/// EnemyHpBar の root + frame + bg + 1 本の gauge sprite を spawn する。Phase 2 では
/// 常に **単一 gauge** (gauge_step を無視) で構築する。target が解決できる間だけ
/// Visibility::Visible になり、target 不在のときは update system が Hidden にする。
fn spawn_enemy_hp_bar(
    commands: &mut Commands,
    parent: Entity,
    root_translation: Vec3,
    cfg: &EnemyHpBarConfig,
) -> Entity {
    let outer_size = Vec2::new(cfg.size.w, cfg.size.h);
    let frame_t = cfg.frame.thickness.max(0.0);
    let inner_size = Vec2::new(
        (cfg.size.w - 2.0 * frame_t).max(0.0),
        (cfg.size.h - 2.0 * frame_t).max(0.0),
    );

    // 初期は Hidden (target が見つかってから Visible に切替)。
    let root = commands
        .spawn((
            HudRoot,
            EnemyHpBarRoot {
                target: cfg.target.clone(),
                fill_direction: cfg.fill_direction,
                inner_size,
            },
            Transform::from_translation(root_translation),
            Visibility::Hidden,
            ChildOf(parent),
        ))
        .id();

    if frame_t > 0.0 {
        commands.spawn((
            Sprite::from_color(Color::from(cfg.frame.color), outer_size),
            Anchor::TOP_LEFT,
            Transform::from_xyz(0.0, 0.0, 0.0),
            ChildOf(root),
        ));
    }

    commands.spawn((
        Sprite::from_color(Color::from(cfg.bg_color), inner_size),
        Anchor::TOP_LEFT,
        Transform::from_xyz(frame_t, -frame_t, 0.1),
        ChildOf(root),
    ));

    // 単一 gauge sprite (FixedCount(1) 相当)。fill_direction に従って anchor を選ぶ。
    let anchor = match cfg.fill_direction {
        FillDirection::LeftToRight | FillDirection::TopToBottom => Anchor::TOP_LEFT,
        FillDirection::RightToLeft => Anchor::TOP_RIGHT,
        FillDirection::BottomToTop => Anchor::BOTTOM_LEFT,
    };
    let origin = match cfg.fill_direction {
        FillDirection::LeftToRight | FillDirection::TopToBottom => {
            Vec3::new(frame_t, -frame_t, 0.2)
        }
        FillDirection::RightToLeft => Vec3::new(cfg.size.w - frame_t, -frame_t, 0.2),
        FillDirection::BottomToTop => Vec3::new(frame_t, -(cfg.size.h - frame_t), 0.2),
    };
    commands.spawn((
        Sprite::from_color(Color::from(cfg.fg_color), inner_size),
        anchor,
        Transform::from_translation(origin),
        EnemyHpBarGauge {
            full_size: inner_size,
            fill_direction: cfg.fill_direction,
        },
        ChildOf(root),
    ));
    root
}

struct GaugeSegment {
    /// HudRoot 直下の inner 領域 (frame オフセット適用前) の TOP_LEFT を原点とした座標。
    /// spawn 時に frame_t 分を足し込む。anchor 種類によって意味が違う:
    /// TOP_LEFT/TOP_RIGHT は sprite の top edge、BOTTOM_LEFT は sprite の bottom edge。
    origin: Vec2,
    full_size: Vec2,
    anchor: Anchor,
    hp_low: f32,
    hp_high: f32,
}

fn gauge_layout(
    step: GaugeStep,
    direction: FillDirection,
    gauge_gap: f32,
    inner: Vec2,
    max_hp: f32,
) -> Vec<GaugeSegment> {
    let ranges = gauge_hp_ranges(step, max_hp);
    let num = ranges.len();
    if num == 0 {
        return Vec::new();
    }

    let gap = gauge_gap.max(0.0);
    let total_gap = gap * (num.saturating_sub(1) as f32);
    let is_horizontal = matches!(
        direction,
        FillDirection::LeftToRight | FillDirection::RightToLeft
    );
    let segment_main = if is_horizontal {
        ((inner.x - total_gap) / num as f32).max(0.0)
    } else {
        ((inner.y - total_gap) / num as f32).max(0.0)
    };

    let anchor = match direction {
        FillDirection::LeftToRight | FillDirection::TopToBottom => Anchor::TOP_LEFT,
        FillDirection::RightToLeft => Anchor::TOP_RIGHT,
        FillDirection::BottomToTop => Anchor::BOTTOM_LEFT,
    };

    ranges
        .into_iter()
        .enumerate()
        .map(|(i, (hp_low, hp_high))| {
            let step_main = segment_main + gap;
            let (origin, full_size) = match direction {
                FillDirection::LeftToRight => (
                    Vec2::new(step_main * i as f32, 0.0),
                    Vec2::new(segment_main, inner.y),
                ),
                FillDirection::RightToLeft => (
                    Vec2::new(inner.x - step_main * i as f32, 0.0),
                    Vec2::new(segment_main, inner.y),
                ),
                FillDirection::TopToBottom => (
                    Vec2::new(0.0, -step_main * i as f32),
                    Vec2::new(inner.x, segment_main),
                ),
                FillDirection::BottomToTop => (
                    Vec2::new(0.0, -inner.y + step_main * i as f32),
                    Vec2::new(inner.x, segment_main),
                ),
            };
            GaugeSegment {
                origin,
                full_size,
                anchor,
                hp_low,
                hp_high,
            }
        })
        .collect()
}

/// `gauge_step` と max HP から各 gauge segment の HP 範囲 `(low, high)` を返す。
/// 順序は HP 範囲の昇順 (= gauge 0 が一番最後に減る gauge)。
// max_hp > 0 を上で確認済み、ceil() 後の u32 化なので truncation/sign loss は起きない
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn gauge_hp_ranges(step: GaugeStep, max_hp: f32) -> Vec<(f32, f32)> {
    if max_hp <= 0.0 {
        return Vec::new();
    }
    match step {
        GaugeStep::FixedCount(n) => {
            let n = n.max(1);
            let per = max_hp / n as f32;
            (0..n)
                .map(|i| (per * i as f32, per * (i + 1) as f32))
                .collect()
        }
        GaugeStep::PerUnit(n) => {
            if n == 0 {
                return Vec::new();
            }
            let per = n as f32;
            let num = ((max_hp / per).ceil() as u32).max(1);
            (0..num)
                .map(|i| {
                    let low = per * i as f32;
                    let high = (per * (i + 1) as f32).min(max_hp);
                    (low, high)
                })
                .collect()
        }
    }
}

/// 画面 anchor + offset から、要素の **top-left** 隅の camera 相対 world 座標を返す。
///
/// world Y は上が正なので、画面感覚の offset.y (下方向が正) は符号反転する。
fn top_left_of_element(anchor: HudAnchor, offset: HudOffset, viewport: (f32, f32)) -> Vec2 {
    let (vw, vh) = viewport;
    let (ax, ay) = anchor_world_pos(anchor, vw, vh);
    Vec2::new(ax + offset.x, ay - offset.y)
}

fn update_player_hp_ring(
    player_query: Query<(&PlayerId, &HitPoints), Changed<HitPoints>>,
    gauges: Query<(&PlayerHpRingGauge, &Mesh2d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // 各 target Player の current HP を 1 度だけ確定し、その target を持つ ring gauge だけ
    // mesh を差し替える。`Changed<HitPoints>` で gating しているため、HP 変化が無い Player
    // 用の ring は触らない (handle 再 allocate を避ける)。
    for (player, hp) in &player_query {
        let current = hp.current as f32;
        for (gauge, mesh2d) in &gauges {
            if gauge.target != *player {
                continue;
            }
            let denom = gauge.hp_high - gauge.hp_low;
            let ratio = if denom <= 0.0 {
                0.0
            } else {
                ((current - gauge.hp_low) / denom).clamp(0.0, 1.0)
            };
            let span = gauge.full_end_rad - gauge.full_start_rad;
            let cur_end = gauge.full_start_rad + span * ratio;
            let new_mesh = build_annular_sector_mesh(
                gauge.outer_r,
                gauge.inner_r,
                gauge.full_start_rad,
                cur_end,
            );
            let _ = meshes.insert(&mesh2d.0, new_mesh);
        }
    }
}

// ring mesh の build と frame / bg / 弦 sprite / segment spawn の連結で 100 行を僅かに
// 越える。1 つの annular sector 構築は一望できる方が読みやすいので分割せず allow する。
#[allow(clippy::too_many_lines)]
fn spawn_player_hp_ring(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    parent: Entity,
    root_translation: Vec3,
    cfg: &PlayerHpRingConfig,
    max_hp: f32,
) -> Entity {
    let w = cfg.size.w;
    let h = cfg.size.h;
    // 外接 bbox から内接円の半径を取る (短辺の半分)。w != h なら短辺基準で円を inscribe する。
    let r = (w.min(h) * 0.5).max(0.0);
    let ft = cfg.frame.thickness.max(0.0);
    let rt = cfg.ring_thickness.max(0.0);

    // 内側 ring の幾何。bar と同じ規約で frame は size の内側に食い込ませる。
    let outer_r = (r - ft).max(0.0);
    let inner_r = (outer_r - rt).max(0.0);
    let frame_outer_r = r;
    let frame_inner_r = (inner_r - ft).max(0.0);

    // 呼び出し側は top-left 起点の root_translation を渡してくる。ring の幾何は中心基準
    // なので w/2, -h/2 シフトして root を bbox の中心に置く。
    let center_translation = Vec3::new(
        root_translation.x + w * 0.5,
        root_translation.y - h * 0.5,
        root_translation.z,
    );
    let root = commands
        .spawn((
            HudRoot,
            Transform::from_translation(center_translation),
            Visibility::default(),
            ChildOf(parent),
        ))
        .id();

    let (start_rad, end_rad) =
        clock_deg_to_math_rad_range(cfg.start_angle, cfg.sweep_extent, cfg.direction);

    // 枠 (frame.thickness > 0 のとき、ring 全体を囲む外形の annular sector を frame 色で塗る)。
    if ft > 0.0 && frame_outer_r > frame_inner_r {
        let mesh = meshes.add(build_annular_sector_mesh(
            frame_outer_r,
            frame_inner_r,
            start_rad,
            end_rad,
        ));
        let mat = materials.add(ColorMaterial::from(Color::from(cfg.frame.color)));
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(mat),
            Transform::from_xyz(0.0, 0.0, 0.0),
            ChildOf(root),
        ));
    }

    // 両端の弦に細長い sprite を 2 枚回転配置 (sweep が 360° 未満で frame があるときだけ)。
    // 弦の中心は frame 全体の半径方向の中央に置き、sprite を radial 方向に向ける。
    let sweep_is_full = (cfg.sweep_extent.abs() - 360.0).abs() < 1e-3;
    if ft > 0.0 && !sweep_is_full && frame_outer_r > frame_inner_r {
        let chord_len = frame_outer_r - frame_inner_r;
        let chord_mid_r = (frame_outer_r + frame_inner_r) * 0.5;
        for &edge_rad in &[start_rad, end_rad] {
            let (sin, cos) = edge_rad.sin_cos();
            commands.spawn((
                Sprite::from_color(Color::from(cfg.frame.color), Vec2::new(chord_len, ft)),
                Transform {
                    translation: Vec3::new(cos * chord_mid_r, sin * chord_mid_r, 0.05),
                    rotation: Quat::from_rotation_z(edge_rad),
                    ..default()
                },
                ChildOf(root),
            ));
        }
    }

    // 内側 bg (ring 全体)。
    if outer_r > 0.0 {
        let mesh = meshes.add(build_annular_sector_mesh(
            outer_r, inner_r, start_rad, end_rad,
        ));
        let mat = materials.add(ColorMaterial::from(Color::from(cfg.bg_color)));
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(mat),
            Transform::from_xyz(0.0, 0.0, 0.1),
            ChildOf(root),
        ));
    }

    // 各 segment の fg。
    let segments = ring_gauge_layout(
        cfg.gauge_step,
        cfg.direction,
        cfg.gauge_gap.to_radians(),
        start_rad,
        end_rad,
        max_hp,
    );
    for seg in segments {
        let mesh = meshes.add(build_annular_sector_mesh(
            outer_r,
            inner_r,
            seg.start_rad,
            seg.end_rad,
        ));
        let mat = materials.add(ColorMaterial::from(Color::from(cfg.fg_color)));
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(mat),
            Transform::from_xyz(0.0, 0.0, 0.2),
            PlayerHpRingGauge {
                target: cfg.target,
                hp_low: seg.hp_low,
                hp_high: seg.hp_high,
                outer_r,
                inner_r,
                full_start_rad: seg.start_rad,
                full_end_rad: seg.end_rad,
            },
            ChildOf(root),
        ));
    }
    root
}

/// 「12時 = 0°、時計回り正」(度) を「3時 = 0°、反時計回り正」(rad) に変換し、
/// `direction` に応じて符号付き sweep を返す。mesh builder には数学標準の角度を渡す。
fn clock_deg_to_math_rad_range(
    start_deg: f32,
    sweep_deg: f32,
    direction: RingDirection,
) -> (f32, f32) {
    let start_math = (90.0 - start_deg).to_radians();
    let signed_sweep_deg = match direction {
        RingDirection::Clockwise => -sweep_deg,
        RingDirection::CounterClockwise => sweep_deg,
    };
    (start_math, start_math + signed_sweep_deg.to_radians())
}

struct RingSegment {
    hp_low: f32,
    hp_high: f32,
    start_rad: f32,
    end_rad: f32,
}

fn ring_gauge_layout(
    step: GaugeStep,
    direction: RingDirection,
    gauge_gap_rad: f32,
    start_rad: f32,
    end_rad: f32,
    max_hp: f32,
) -> Vec<RingSegment> {
    let ranges = gauge_hp_ranges(step, max_hp);
    let num = ranges.len();
    if num == 0 {
        return Vec::new();
    }
    let signed_sweep = end_rad - start_rad;
    // gap も sweep と同符号で進む向きに置く。
    let signed_gap = match direction {
        RingDirection::Clockwise => -gauge_gap_rad.max(0.0),
        RingDirection::CounterClockwise => gauge_gap_rad.max(0.0),
    };
    let total_gap = signed_gap * (num.saturating_sub(1) as f32);
    let segment_signed = (signed_sweep - total_gap) / num as f32;

    // ranges は HP 範囲の昇順 = ranges[0] が最後に消える gauge。bar と同じ規約で
    // 「終端側の segment から減る」ため、始端 (i=0) には ranges[num-1] (最も高い HP 範囲) を置く。
    (0..num)
        .map(|i| {
            let seg_start = start_rad + (segment_signed + signed_gap) * i as f32;
            let seg_end = seg_start + segment_signed;
            let (hp_low, hp_high) = ranges[num - 1 - i];
            RingSegment {
                hp_low,
                hp_high,
                start_rad: seg_start,
                end_rad: seg_end,
            }
        })
        .collect()
}

/// `outer_r` から `inner_r` までの帯を `start_rad..end_rad` (rad) で覆う annular sector mesh。
/// 角度は数学標準 (3時 = 0°、反時計回り正)。`inner_r == 0` のときは扇形になる。
// steps は 0..720 (= 2 周ぶん) で u32 範囲内、indices も 2*(steps+1) < u32::MAX。
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn build_annular_sector_mesh(outer_r: f32, inner_r: f32, start_rad: f32, end_rad: f32) -> Mesh {
    let span = end_rad - start_rad;
    // 1° あたり 1 step、最低 2 step。0° のときも degenerate 三角を含む空 mesh で返す。
    let abs_span_deg = span.abs().to_degrees().clamp(0.0, 720.0);
    let steps = (abs_span_deg.ceil() as usize).max(2);

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((steps + 1) * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity((steps + 1) * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity((steps + 1) * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(steps * 6);

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let angle = start_rad + span * t;
        let (s, c) = angle.sin_cos();
        positions.push([c * outer_r, s * outer_r, 0.0]);
        positions.push([c * inner_r, s * inner_r, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([t, 0.0]);
        uvs.push([t, 1.0]);
    }

    for i in 0..steps {
        let o0 = (2 * i) as u32;
        let i0 = (2 * i + 1) as u32;
        let o1 = (2 * (i + 1)) as u32;
        let i1 = (2 * (i + 1) + 1) as u32;
        indices.extend_from_slice(&[o0, i0, o1, o1, i0, i1]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// EnemyHpBar の毎 frame 更新 (ADR-0031)。
///
/// 各 EnemyHpBarRoot について target を解決:
/// - resolve できない (= 該当 enemy が居ない) → root の Visibility::Hidden で消す
/// - resolve できた → Visibility::Visible にして、その enemy の current/max HP で gauge sprite を縮める
///
/// 子の gauge sprite は HudRoot 配下に 1 個だけ (Phase 2 仕様)。
fn update_enemy_hp_bar(
    mut roots: Query<(&EnemyHpBarRoot, &Children, &mut Visibility)>,
    // ADR-0038: 旧 `With<Enemy>` は `&Side` 値判定で `Side::Villain` 限定に置換 (= 旧 Enemy
    // entity と等価)。Hero side の HUD 起点は別 query (player_query / PlayerId 引き)。
    enemy_query: Query<(Entity, Option<&EnemyTag>, &HitPoints, &Side)>,
    player_query: Query<(&PlayerId, &LastEngagedWith)>,
    mut gauges: Query<(&EnemyHpBarGauge, &mut Sprite)>,
) {
    for (root, children, mut visibility) in &mut roots {
        let resolved = resolve_enemy_target(&root.target, &enemy_query, &player_query);
        let Some((_, hp)) = resolved else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        let current = hp.current as f32;
        let max = hp.max as f32;
        let ratio = if max <= 0.0 {
            0.0
        } else {
            (current / max).clamp(0.0, 1.0)
        };
        // 子 gauge sprite を縮める。EnemyHpBarRoot の子は frame / bg / gauge と 3 個ぶら
        // 下がるが、`EnemyHpBarGauge` が attach されているのは gauge sprite 1 個だけ。
        for child in children.iter() {
            let Ok((gauge, mut sprite)) = gauges.get_mut(child) else {
                continue;
            };
            let Some(size) = sprite.custom_size.as_mut() else {
                continue;
            };
            match gauge.fill_direction {
                FillDirection::LeftToRight | FillDirection::RightToLeft => {
                    size.x = gauge.full_size.x * ratio;
                }
                FillDirection::TopToBottom | FillDirection::BottomToTop => {
                    size.y = gauge.full_size.y * ratio;
                }
            }
        }
    }
}

/// `EnemyTarget` を現状の Villain (= 旧 Enemy) 群から resolve し、(entity, hp) を返す。
/// 該当が無ければ `None`。複数候補がある場合は最初に見つかったものを採用 (Bevy の
/// query iteration 順、entity 生成順に近い)。ADR-0038: `&Side` 値判定で Villain 限定。
fn resolve_enemy_target<'w>(
    target: &EnemyTarget,
    enemy_query: &'w Query<(Entity, Option<&EnemyTag>, &HitPoints, &Side)>,
    player_query: &Query<(&PlayerId, &LastEngagedWith)>,
) -> Option<(Entity, &'w HitPoints)> {
    let villains = || {
        enemy_query
            .iter()
            .filter(|(_, _, _, s)| matches!(s, Side::Villain))
    };
    match target {
        EnemyTarget::LastEngagedBy(pid) => {
            let (_, last) = player_query.iter().find(|(p, _)| **p == *pid)?;
            let target_entity = last.0?;
            villains()
                .find(|(e, _, _, _)| *e == target_entity)
                .map(|(e, _, hp, _)| (e, hp))
        }
        EnemyTarget::Tag(tag) => villains()
            .find(|(_, t, _, _)| t.is_some_and(|t| t.0 == *tag))
            .map(|(e, _, hp, _)| (e, hp)),
        EnemyTarget::NthEnemy(n) => villains().nth(*n).map(|(e, _, hp, _)| (e, hp)),
    }
}

/// ADR-0032 / ADR-0038: Villain entity が新規に登場したとき (`Added<Side>` + `Side::Villain`
/// 値判定)、project.hud.elements の中で `enemy_overhead_hp_bar` に該当する config を全部走査
/// し、`tag_filter` に合致するものを各 Villain entity の child として spawn する。Villain が
/// despawn されると Bevy hierarchy で子もまとめて消える。
/// `Added<Side>` は Hero spawn でも発火するが、内側の `matches!(side, Side::Villain)` で skip。
fn spawn_enemy_overhead_hp_bars(
    mut commands: Commands,
    project: Option<Res<Project>>,
    new_enemies: Query<(Entity, Option<&EnemyTag>, &AnimationFrames, &Side), Added<Side>>,
) {
    let Some(project) = project else {
        return;
    };
    if new_enemies.is_empty() {
        return;
    }
    let overhead_cfgs: Vec<&EnemyOverheadHpBarConfig> = project
        .hud
        .elements
        .iter()
        .filter_map(|e| match e {
            HudElement::EnemyOverheadHpBar(c) => Some(c),
            _ => None,
        })
        .collect();
    if overhead_cfgs.is_empty() {
        return;
    }

    for (enemy_entity, tag, anim, side) in &new_enemies {
        if !matches!(side, Side::Villain) {
            continue;
        }
        for cfg in &overhead_cfgs {
            // tag_filter が Some なら EnemyTag が一致するときだけ attach。None なら全 villain。
            if let Some(filter) = &cfg.tag_filter {
                let matches = tag.is_some_and(|t| t.0 == *filter);
                if !matches {
                    continue;
                }
            }
            spawn_overhead_bar(&mut commands, enemy_entity, cfg, anim);
        }
    }
}

/// `vertical_anchor` と `offset_y` と現フレームの sprite 情報から、Enemy 局所 Y を返す。
/// `image_top` / `image_bottom` は AnimationFrames に問い合わせて毎 frame 変化を反映する。
fn overhead_local_y(anchor: OverheadVerticalAnchor, offset_y: f32, anim: &AnimationFrames) -> f32 {
    // sprite pivot[1] は画像上端から pivot までの距離 (px、下向き)。
    // bevy 局所 Y では pivot (= Enemy origin) から上方向に +pivot_y 進むと画像上端。
    let pivot_y = anim.current_sprite_pivot()[1] as f32;
    let image_h = anim.current_image_dims()[1] as f32;
    match anchor {
        OverheadVerticalAnchor::Origin => offset_y,
        OverheadVerticalAnchor::ImageTop => pivot_y + offset_y,
        // 画像下端 = origin から (image_h - pivot_y) 下方向 = bevy Y で -(image_h - pivot_y)。
        OverheadVerticalAnchor::ImageBottom => -(image_h - pivot_y) + offset_y,
    }
}

/// 1 個の overhead bar (root + frame + bg + gauge) を Enemy entity の子として spawn する。
/// 位置は `vertical_anchor` + `offset_y` で決まり、毎 frame `update_enemy_overhead_hp_bar`
/// が再計算する。**外形 bbox は X 中央**起点なので、root の中心が Enemy の真上に来る。
fn spawn_overhead_bar(
    commands: &mut Commands,
    enemy: Entity,
    cfg: &EnemyOverheadHpBarConfig,
    anim: &AnimationFrames,
) {
    let outer_size = Vec2::new(cfg.size.w, cfg.size.h);
    let frame_t = cfg.frame.thickness.max(0.0);
    let inner_size = Vec2::new(
        (cfg.size.w - 2.0 * frame_t).max(0.0),
        (cfg.size.h - 2.0 * frame_t).max(0.0),
    );

    // 初期 Y を spawn 時点の sprite 情報から計算 (update が毎 frame 上書きするが、1 frame
    // 目から正しい位置にしておくため)。
    let initial_y = overhead_local_y(cfg.vertical_anchor, cfg.offset_y, anim);
    // local Z は 10 (= キャラ sprite より少し手前) で sprite の前に出すが、HUD_Z (= 100,
    // screen HUD 用) よりは奥にする。
    let root_translation = Vec3::new(0.0, initial_y, 10.0);
    let root = commands
        .spawn((
            HudRoot,
            EnemyOverheadHpBarRoot {
                enemy,
                vertical_anchor: cfg.vertical_anchor,
                offset_y: cfg.offset_y,
            },
            Transform::from_translation(root_translation),
            Visibility::default(),
            ChildOf(enemy),
        ))
        .id();

    // 中央起点で sprite を置くため Anchor::Center をデフォルトにし、frame は outer 全面、
    // bg / gauge は inner を「左端詰め」で描いて fill_direction に合わせる。
    if frame_t > 0.0 {
        commands.spawn((
            Sprite::from_color(Color::from(cfg.frame.color), outer_size),
            Anchor::CENTER,
            Transform::from_xyz(0.0, 0.0, 0.0),
            ChildOf(root),
        ));
    }
    // bg は inner を中央配置。
    commands.spawn((
        Sprite::from_color(Color::from(cfg.bg_color), inner_size),
        Anchor::CENTER,
        Transform::from_xyz(0.0, 0.0, 0.1),
        ChildOf(root),
    ));
    // gauge sprite。fill_direction に応じて anchor を端に寄せ、縮みは sprite custom_size の
    // X を current/max で scale して表現する (Player bar の単一 gauge と同じ規約)。
    let (anchor, origin_x) = match cfg.fill_direction {
        FillDirection::LeftToRight | FillDirection::TopToBottom => {
            (Anchor::CENTER_LEFT, -inner_size.x * 0.5)
        }
        FillDirection::RightToLeft => (Anchor::CENTER_RIGHT, inner_size.x * 0.5),
        FillDirection::BottomToTop => (Anchor::CENTER_LEFT, -inner_size.x * 0.5),
    };
    commands.spawn((
        Sprite::from_color(Color::from(cfg.fg_color), inner_size),
        anchor,
        Transform::from_xyz(origin_x, 0.0, 0.2),
        EnemyOverheadHpBarGauge {
            full_size: inner_size,
            fill_direction: cfg.fill_direction,
        },
        ChildOf(root),
    ));
}

/// 毎 frame、各 overhead bar root について:
/// 1. 親 Enemy の AnimationFrames から現フレームの sprite 情報を読み、`vertical_anchor` +
///    `offset_y` に基づいて bar root の Transform.Y を再計算する (image_top / image_bottom
///    なら sprite 高さに追従)。
/// 2. HitPoints から ratio を取り、gauge の custom_size を縮める。
///
/// Enemy が既に despawn されていたら Bevy hierarchy で root も既に消えているはずだが、
/// 念のため query.get で None を skip する。
fn update_enemy_overhead_hp_bar(
    // ADR-0038: 旧 `With<Enemy>` は `&Side` 値判定で Villain 限定。`enemy_query.get(...)` で
    // 引いた entity が Hero side だった場合 (= overhead bar attach されない想定だが念のため)
    // は skip する。
    enemy_query: Query<(&HitPoints, &AnimationFrames, &Side)>,
    mut roots: Query<(&EnemyOverheadHpBarRoot, &mut Transform, &Children)>,
    mut gauges: Query<(&EnemyOverheadHpBarGauge, &mut Sprite)>,
) {
    for (root, mut transform, children) in &mut roots {
        let Ok((hp, anim, vic_side)) = enemy_query.get(root.enemy) else {
            continue;
        };
        if !matches!(vic_side, Side::Villain) {
            continue;
        }
        // Y を毎 frame 再計算 (sprite 形状が変わると image_top/bottom が動くため)。
        let new_y = overhead_local_y(root.vertical_anchor, root.offset_y, anim);
        if (transform.translation.y - new_y).abs() > f32::EPSILON {
            transform.translation.y = new_y;
        }

        let current = hp.current as f32;
        let max = hp.max as f32;
        let ratio = if max <= 0.0 {
            0.0
        } else {
            (current / max).clamp(0.0, 1.0)
        };
        for child in children.iter() {
            let Ok((gauge, mut sprite)) = gauges.get_mut(child) else {
                continue;
            };
            let Some(size) = sprite.custom_size.as_mut() else {
                continue;
            };
            match gauge.fill_direction {
                FillDirection::LeftToRight | FillDirection::RightToLeft => {
                    size.x = gauge.full_size.x * ratio;
                }
                FillDirection::TopToBottom | FillDirection::BottomToTop => {
                    size.y = gauge.full_size.y * ratio;
                }
            }
        }
    }
}

/// PlayerIcon の root marker (ADR-0033)。CharacterState 変化で sprite を hot swap するための
/// `icons_by_role` map と shake trigger / params を保持する。
///
/// `last_hp` は `detect_icon_damage` が「current HP が前 frame より減ったか」を判定するために
/// 直前 frame の値を覚えるための一時変数。spawn 時に Player の現 HP で初期化する。
#[derive(Component)]
struct PlayerIconRoot {
    target: PlayerId,
    icons_by_role: HashMap<Role, Handle<Image>>,
    default_handle: Handle<Image>,
    on_damage: Option<IconShakeParams>,
    on_attack_hit: Option<IconShakeParams>,
    last_hp: u32,
}

/// PlayerIcon の中央 sprite (= 実際に Image を表示する子)。state 切替の対象。
#[derive(Component)]
struct PlayerIconSprite;

/// 振動中の Icon root に attach される component (ADR-0033)。
/// HitStop と同じ三角波 + 線形減衰モデル。`base_translation` は shake offset 適用前の
/// root.translation (= spawn 時の位置)。tick_icon_shake が毎 frame `base + offset` で
/// Transform を上書きする。
#[derive(Component, Clone, Copy)]
struct IconShakeState {
    total_ms: u32,
    remaining_ms: f32,
    shake_x: i32,
    shake_y: i32,
    count: u32,
    decay: f32,
    base_translation: Vec3,
}

impl IconShakeState {
    fn from_params(params: IconShakeParams, base_translation: Vec3) -> Self {
        Self {
            total_ms: params.duration_ms,
            remaining_ms: params.duration_ms as f32,
            shake_x: params.shake_x,
            shake_y: params.shake_y,
            count: params.count,
            decay: params.decay,
            base_translation,
        }
    }
}

/// PlayerIcon の root + frame + bg + 中央 sprite を spawn する (ADR-0033)。
///
/// state_sprites の各 Role について sprite_index を引いて asset_server で Image を pre-load し、
/// PlayerIconRoot の icons_by_role に貯める。CharacterState 変化時の sprite swap は
/// `update_player_icon_sprite` が icons_by_role から該当 Role の handle を引き直して
/// 子 sprite の image を差し替える (Image 自体は事前 load 済みなので IO 無し)。
///
/// 該当 sprite_group_number が見つからない / sprite_index が group 内に存在しない場合は warn
/// を出して該当 Role を skip する。default_handle だけ確実に取れていれば icon は表示される。
#[allow(clippy::too_many_arguments)]
fn spawn_player_icon(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    root_translation: Vec3,
    cfg: &PlayerIconConfig,
    player_groups: &crate::features::character::PlayerSpriteGroups,
    player_query: &Query<(&PlayerId, &HitPoints)>,
) -> Option<Entity> {
    let outer_size = Vec2::new(cfg.size.w, cfg.size.h);
    let frame_t = cfg.frame.thickness.max(0.0);
    let inner_size = Vec2::new(
        (cfg.size.w - 2.0 * frame_t).max(0.0),
        (cfg.size.h - 2.0 * frame_t).max(0.0),
    );

    let Some(group) = player_groups.sprite_groups.get(&cfg.sprite_group_number) else {
        tracing::warn!(
            target = ?cfg.target,
            sprite_group_number = cfg.sprite_group_number,
            "hud: sprite_group_number not found in player's sprite_groups, skipping player_icon",
        );
        return None;
    };
    let load_handle = |sprite_index: u32| -> Option<Handle<Image>> {
        let sprite = group.sprites.iter().find(|s| s.index == sprite_index)?;
        let asset_rel = format!(
            "characters/{}/sprite-groups/{}/sprites/{}",
            player_groups.character_name, group.name, sprite.path,
        );
        Some(asset_server.load(asset_rel))
    };
    let Some(default_handle) = load_handle(cfg.default_sprite_index) else {
        tracing::warn!(
            target = ?cfg.target,
            sprite_group_number = cfg.sprite_group_number,
            default_sprite_index = cfg.default_sprite_index,
            "hud: default_sprite_index not found in sprite group, skipping player_icon",
        );
        return None;
    };
    let mut icons_by_role: HashMap<Role, Handle<Image>> = HashMap::new();
    for (role, sprite_index) in &cfg.state_sprites {
        let Some(handle) = load_handle(*sprite_index) else {
            tracing::warn!(
                ?role,
                sprite_index,
                group = %group.name,
                "hud: sprite_index not found in icon sprite group, falling back to default",
            );
            continue;
        };
        icons_by_role.insert(*role, handle);
    }

    let initial_hp = player_query
        .iter()
        .find(|(p, _)| **p == cfg.target)
        .map_or(0, |(_, hp)| hp.current);

    let root = commands
        .spawn((
            HudRoot,
            PlayerIconRoot {
                target: cfg.target,
                icons_by_role,
                default_handle: default_handle.clone(),
                on_damage: cfg.shake.on_damage,
                on_attack_hit: cfg.shake.on_attack_hit,
                last_hp: initial_hp,
            },
            Transform::from_translation(root_translation),
            Visibility::default(),
            ChildOf(parent),
        ))
        .id();

    if frame_t > 0.0 {
        commands.spawn((
            Sprite::from_color(Color::from(cfg.frame.color), outer_size),
            Anchor::TOP_LEFT,
            Transform::from_xyz(0.0, 0.0, 0.0),
            ChildOf(root),
        ));
    }
    // bg は Icon 用途では default 完全透明 (= 描かない見た目) だが、bg_color を設定すれば
    // sprite の後ろに塗れる。HP bar と同じ TOP_LEFT 基準で inner を埋める。
    if cfg.bg_color.a > 0 {
        commands.spawn((
            Sprite::from_color(Color::from(cfg.bg_color), inner_size),
            Anchor::TOP_LEFT,
            Transform::from_xyz(frame_t, -frame_t, 0.1),
            ChildOf(root),
        ));
    }

    // Icon 本体 (中央配置)。custom_size で size に fit させて、画像の素サイズに依らず
    // 同じ HUD 寸法を保つ。
    let icon_pos = Vec3::new(cfg.size.w * 0.5, -cfg.size.h * 0.5, 0.2);
    commands.spawn((
        Sprite {
            image: default_handle,
            custom_size: Some(inner_size),
            ..default()
        },
        Anchor::CENTER,
        Transform::from_translation(icon_pos),
        PlayerIconSprite,
        ChildOf(root),
    ));

    Some(root)
}

/// Player の CharacterState が変化したら、icons_by_role から該当 Role の Image handle を
/// 引いて子 sprite の image を差し替える。state_sprites に未登録の Role は default_handle に
/// フォールバックする。
fn update_player_icon_sprite(
    roots: Query<(&PlayerIconRoot, &Children)>,
    player_query: Query<(&PlayerId, &CharacterState), Changed<CharacterState>>,
    mut sprites: Query<&mut Sprite, With<PlayerIconSprite>>,
) {
    for (player, state) in &player_query {
        let role = state.to_role();
        for (root, children) in &roots {
            if root.target != *player {
                continue;
            }
            let handle = root
                .icons_by_role
                .get(&role)
                .unwrap_or(&root.default_handle)
                .clone();
            for child in children.iter() {
                if let Ok(mut sprite) = sprites.get_mut(child) {
                    sprite.image = handle.clone();
                }
            }
        }
    }
}

/// 各 PlayerIconRoot について、target Player の HitPoints の current が前 frame より
/// 減っていたら on_damage の振動を発火する。`last_hp` を毎 frame 更新する。
fn detect_icon_damage(
    mut commands: Commands,
    mut roots: Query<(Entity, &mut PlayerIconRoot, &Transform)>,
    player_query: Query<(&PlayerId, &HitPoints)>,
) {
    for (entity, mut root, transform) in &mut roots {
        let Some((_, hp)) = player_query.iter().find(|(p, _)| **p == root.target) else {
            continue;
        };
        if hp.current < root.last_hp
            && let Some(params) = root.on_damage
        {
            commands
                .entity(entity)
                .insert(IconShakeState::from_params(params, transform.translation));
        }
        root.last_hp = hp.current;
    }
}

/// Player に HitStopState が新規 attach された瞬間 (= attack が hit を出した瞬間、
/// attack.rs の resolve_hits が attacker(...) で attach) を Added で拾い、
/// 対応する PlayerIconRoot に on_attack_hit の振動を仕込む。
fn detect_icon_attack_hit(
    mut commands: Commands,
    roots: Query<(Entity, &PlayerIconRoot, &Transform)>,
    new_hit: Query<&PlayerId, Added<HitStopState>>,
) {
    for player in &new_hit {
        for (entity, root, transform) in &roots {
            if root.target != *player {
                continue;
            }
            let Some(params) = root.on_attack_hit else {
                continue;
            };
            commands
                .entity(entity)
                .insert(IconShakeState::from_params(params, transform.translation));
        }
    }
}

/// IconShakeState が attach されている root について、HitStop と同じ三角波 + 線形減衰で
/// Transform.translation に offset を載せる。remaining が 0 を切ったら component を remove し、
/// base_translation に戻す。
fn tick_icon_shake(
    mut commands: Commands,
    time: Res<Time>,
    mut shaking: Query<(Entity, &mut IconShakeState, &mut Transform)>,
) {
    let dt_ms = time.delta_secs() * 1000.0;
    for (entity, mut state, mut transform) in &mut shaking {
        let (off_x, off_y) = if state.count > 0 && state.total_ms > 0 {
            let progress = (1.0 - state.remaining_ms / state.total_ms as f32).clamp(0.0, 1.0);
            let phase = progress * (state.count as f32) * 0.25;
            let wave = icon_triangle_wave(phase);
            let amp = (1.0 - state.decay * progress).clamp(0.0, 1.0);
            (
                wave * (state.shake_x as f32) * amp,
                wave * (state.shake_y as f32) * amp,
            )
        } else {
            (0.0, 0.0)
        };
        transform.translation = state.base_translation + Vec3::new(off_x, off_y, 0.0);
        state.remaining_ms -= dt_ms;
        if state.remaining_ms <= 0.0 {
            transform.translation = state.base_translation;
            commands.entity(entity).remove::<IconShakeState>();
        }
    }
}

/// 三角波 (周期 1)。hit_stop の triangle_wave と同じ。Icon HUD は別 slice に閉じて持ちたいので
/// 共通化はせず、必要になってから DRY を検討する (CLAUDE.md「先に共通基盤を作らない」)。
#[must_use]
fn icon_triangle_wave(x: f32) -> f32 {
    let frac = x - x.floor();
    if frac < 0.25 {
        frac * 4.0
    } else if frac < 0.75 {
        2.0 - frac * 4.0
    } else {
        frac * 4.0 - 4.0
    }
}

fn anchor_world_pos(anchor: HudAnchor, vw: f32, vh: f32) -> (f32, f32) {
    let half_w = vw * 0.5;
    let half_h = vh * 0.5;
    match anchor {
        HudAnchor::TopLeft => (-half_w, half_h),
        HudAnchor::Top => (0.0, half_h),
        HudAnchor::TopRight => (half_w, half_h),
        HudAnchor::Left => (-half_w, 0.0),
        HudAnchor::Center => (0.0, 0.0),
        HudAnchor::Right => (half_w, 0.0),
        HudAnchor::BottomLeft => (-half_w, -half_h),
        HudAnchor::Bottom => (0.0, -half_h),
        HudAnchor::BottomRight => (half_w, -half_h),
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn top_left_at_top_left_anchor_is_screen_top_left_minus_offset_y() {
        // 384x216 の画面で top_left anchor + offset(16, 16) は world (-176, 92) になる。
        let pos = top_left_of_element(
            HudAnchor::TopLeft,
            HudOffset { x: 16.0, y: 16.0 },
            (384.0, 216.0),
        );
        assert_eq!(pos.x, -176.0);
        assert_eq!(pos.y, 92.0);
    }

    #[test]
    fn top_left_at_bottom_right_anchor_uses_positive_x_and_negative_y_origin() {
        let pos = top_left_of_element(
            HudAnchor::BottomRight,
            HudOffset { x: -16.0, y: -16.0 },
            (384.0, 216.0),
        );
        assert_eq!(pos.x, 176.0);
        assert_eq!(pos.y, -92.0);
    }

    #[test]
    fn center_anchor_with_zero_offset_is_world_origin() {
        let pos = top_left_of_element(HudAnchor::Center, HudOffset::default(), (384.0, 216.0));
        assert_eq!(pos, Vec2::ZERO);
    }

    #[test]
    fn gauge_ranges_fixed_count_divides_max_hp_evenly() {
        let ranges = gauge_hp_ranges(GaugeStep::FixedCount(4), 200.0);
        assert_eq!(
            ranges,
            vec![(0.0, 50.0), (50.0, 100.0), (100.0, 150.0), (150.0, 200.0)]
        );
    }

    #[test]
    fn gauge_ranges_per_unit_last_segment_holds_remainder() {
        // max=350, per_unit=100 → 4 segments、最後は HP 50 分だけ持つ。
        let ranges = gauge_hp_ranges(GaugeStep::PerUnit(100), 350.0);
        assert_eq!(
            ranges,
            vec![(0.0, 100.0), (100.0, 200.0), (200.0, 300.0), (300.0, 350.0),]
        );
    }

    #[test]
    fn gauge_ranges_fixed_count_zero_clamps_to_one() {
        let ranges = gauge_hp_ranges(GaugeStep::FixedCount(0), 100.0);
        assert_eq!(ranges, vec![(0.0, 100.0)]);
    }

    #[test]
    fn gauge_ranges_per_unit_zero_returns_empty() {
        let ranges = gauge_hp_ranges(GaugeStep::PerUnit(0), 100.0);
        assert!(ranges.is_empty());
    }

    #[test]
    fn gauge_layout_ltr_places_segments_left_to_right() {
        let segs = gauge_layout(
            GaugeStep::FixedCount(2),
            FillDirection::LeftToRight,
            0.0,
            Vec2::new(100.0, 10.0),
            100.0,
        );
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].origin.x, 0.0);
        assert_eq!(segs[1].origin.x, 50.0);
        assert_eq!(segs[0].full_size, Vec2::new(50.0, 10.0));
    }

    #[test]
    fn gauge_layout_rtl_places_segment_zero_at_right_edge() {
        let segs = gauge_layout(
            GaugeStep::FixedCount(2),
            FillDirection::RightToLeft,
            0.0,
            Vec2::new(100.0, 10.0),
            100.0,
        );
        assert_eq!(segs[0].origin.x, 100.0);
        assert_eq!(segs[1].origin.x, 50.0);
    }

    #[test]
    fn gauge_layout_ttb_places_segment_zero_at_top() {
        let segs = gauge_layout(
            GaugeStep::FixedCount(2),
            FillDirection::TopToBottom,
            0.0,
            Vec2::new(10.0, 100.0),
            100.0,
        );
        assert_eq!(segs[0].origin.y, 0.0);
        assert_eq!(segs[1].origin.y, -50.0);
        assert_eq!(segs[0].full_size, Vec2::new(10.0, 50.0));
    }

    #[test]
    fn gauge_layout_btt_places_segment_zero_at_bottom() {
        let segs = gauge_layout(
            GaugeStep::FixedCount(2),
            FillDirection::BottomToTop,
            0.0,
            Vec2::new(10.0, 100.0),
            100.0,
        );
        assert_eq!(segs[0].origin.y, -100.0);
        assert_eq!(segs[1].origin.y, -50.0);
    }

    #[test]
    fn gauge_layout_subtracts_gap_from_segment_widths() {
        // gap 4 × 2 個分 = 8px、残り 92px を 3 等分 → 各 segment 約 30.67px。
        let segs = gauge_layout(
            GaugeStep::FixedCount(3),
            FillDirection::LeftToRight,
            4.0,
            Vec2::new(100.0, 10.0),
            100.0,
        );
        assert_eq!(segs.len(), 3);
        assert!((segs[0].full_size.x - (92.0 / 3.0)).abs() < 1e-3);
    }

    #[test]
    fn clock_to_math_zero_is_top_clockwise_is_negative() {
        // 12 時起点・時計回り → 数学標準では 90° から負方向に進む。
        let (start, end) = clock_deg_to_math_rad_range(0.0, 90.0, RingDirection::Clockwise);
        assert!((start - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!((end - 0.0).abs() < 1e-5);
    }

    #[test]
    fn clock_to_math_counter_clockwise_progresses_positively() {
        let (start, end) = clock_deg_to_math_rad_range(0.0, 90.0, RingDirection::CounterClockwise);
        assert!((start - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!((end - std::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn ring_gauge_layout_clockwise_terminal_segment_holds_lowest_hp_range() {
        // FixedCount(3), 全周 360°, gap 0 → 各 segment が 120° ぶん。
        // direction=clockwise なら 始端 segment (i=0) に最高 HP range (200..300) を載せる。
        let (start, end) = clock_deg_to_math_rad_range(0.0, 360.0, RingDirection::Clockwise);
        let segs = ring_gauge_layout(
            GaugeStep::FixedCount(3),
            RingDirection::Clockwise,
            0.0,
            start,
            end,
            300.0,
        );
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].hp_low, 200.0);
        assert_eq!(segs[0].hp_high, 300.0);
        assert_eq!(segs[2].hp_low, 0.0);
        assert_eq!(segs[2].hp_high, 100.0);
    }

    #[test]
    fn ring_gauge_layout_subtracts_gap_from_segment_angles() {
        // 360° を 4 等分、gap 5° × 3 = 15° → 残り 345° を 4 等分 = 86.25°/seg
        let (start, end) = clock_deg_to_math_rad_range(0.0, 360.0, RingDirection::Clockwise);
        let segs = ring_gauge_layout(
            GaugeStep::FixedCount(4),
            RingDirection::Clockwise,
            5.0_f32.to_radians(),
            start,
            end,
            400.0,
        );
        let span = (segs[0].end_rad - segs[0].start_rad).abs().to_degrees();
        assert!((span - 86.25).abs() < 1e-3);
    }

    #[test]
    fn icon_triangle_wave_matches_hit_stop_quarters() {
        // ADR-0033: hit_stop の triangle_wave と同じ波形 (x=0→0, 0.25→1, 0.5→0, 0.75→-1, 1→0)。
        assert!((icon_triangle_wave(0.0) - 0.0).abs() < 1e-6);
        assert!((icon_triangle_wave(0.25) - 1.0).abs() < 1e-6);
        assert!((icon_triangle_wave(0.5) - 0.0).abs() < 1e-6);
        assert!((icon_triangle_wave(0.75) - (-1.0)).abs() < 1e-6);
        assert!((icon_triangle_wave(1.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn icon_shake_state_from_params_copies_fields_and_seeds_remaining() {
        let params = IconShakeParams {
            duration_ms: 120,
            shake_x: 3,
            shake_y: 5,
            count: 4,
            decay: 0.5,
        };
        let base = Vec3::new(10.0, -20.0, 30.0);
        let s = IconShakeState::from_params(params, base);
        assert_eq!(s.total_ms, 120);
        assert!((s.remaining_ms - 120.0).abs() < 1e-6);
        assert_eq!(s.shake_x, 3);
        assert_eq!(s.shake_y, 5);
        assert_eq!(s.count, 4);
        assert!((s.decay - 0.5).abs() < 1e-6);
        assert_eq!(s.base_translation, base);
    }

    #[test]
    fn annular_sector_mesh_has_expected_vertex_count() {
        // 90° span → ceil(90)=90 steps、頂点は (90+1)*2 = 182 個。
        let mesh = build_annular_sector_mesh(32.0, 24.0, 0.0, std::f32::consts::FRAC_PI_2);
        let pos = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("has position");
        // VertexAttributeValues::Float32x3 の len() を取りたいので mesh.count_vertices() を使う。
        let n = mesh.count_vertices();
        assert_eq!(n, 182);
        // attribute は touch だけして使われない警告を抑える
        let _ = pos;
    }
}
