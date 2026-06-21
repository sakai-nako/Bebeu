//! State / Frame debug overlay (FSD: feature slice)。
//!
//! `F2` で [`StateDebugEnabled`] を toggle し、有効時に各 [`Player`] / [`Enemy`] entity
//! の頭上に `Text2d` で現在状態を表示する。表示項目:
//!
//! - `CharacterState` (Idle / Walk / Attack / KnockbackUp / ...)
//! - 現フレーム index / 総フレーム数
//! - HP (current/max)
//! - Combatant: gauge / threshold, remaining_bounces, final_action, hit_from_behind
//!
//! ラベルは [`FINAL_PASS_LAYER`] (= [`super::super::super::app::FinalPassCamera`]) に乗せて
//! window 解像度で描画する。scene camera → 中間 texture → linear 拡大の経路を通すと
//! 小さい font が滲んで読めなくなるため。位置は char の scene world 座標を
//! `(scene_world - main_camera_world) * viewport→window scale` で window 中心
//! 基準の coord に変換する。
//! font は Bevy default (default_font feature の FiraMono embedded) を使う。
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::app::{FINAL_PASS_LAYER, PixelPerfectConfig};
use crate::shared::projection;

use super::animation::{AnimationFrames, AnimationSet};
use super::attack::HitPoints;
use super::knockback::{Combatant, PhysicsParams};
use super::movement::{Enemy, MainCamera, Player, WorldPosition};
use super::state_machine::CharacterState;

/// debug overlay の on/off。F2 で toggle。default = off。
#[derive(Resource, Debug, Default)]
pub struct StateDebugEnabled(pub bool);

/// 1 キャラぶんの debug ラベル。`target` で対応する Player / Enemy entity を指す。
/// target が despawn された場合、`update_labels` が自身を despawn して回収する。
#[derive(Component, Debug)]
struct StateDebugLabel {
    target: Entity,
}

pub struct StateDebugPlugin;

impl Plugin for StateDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateDebugEnabled>().add_systems(
            Update,
            (toggle_debug, ensure_labels, update_labels)
                .chain()
                // anim.current_index() / current_body_boxes() は tick 後の状態を読みたい。
                .after(AnimationSet::Tick),
        );
    }
}

fn toggle_debug(keys: Res<ButtonInput<KeyCode>>, mut enabled: ResMut<StateDebugEnabled>) {
    if keys.just_pressed(KeyCode::F2) {
        enabled.0 = !enabled.0;
        tracing::info!(enabled = enabled.0, "state debug: toggled");
    }
}

/// 各 Player / Enemy entity に対応する `StateDebugLabel` がまだ無ければ spawn する。
/// disabled 中も label entity は生かしておき、`Visibility` で見せ隠しする (毎回 spawn /
/// despawn すると ECS の archetype 遷移が忙しくなる)。
///
/// ラベルは `FINAL_PASS_LAYER` に配置して FinalPassCamera で描画 → window 解像度で
/// crisp に出る。font_size は window 画素単位なので 14px で十分視認できる。
#[allow(clippy::needless_pass_by_value)]
fn ensure_labels(
    mut commands: Commands,
    targets: Query<Entity, Or<(With<Player>, With<Enemy>)>>,
    labels: Query<&StateDebugLabel>,
) {
    let labeled: std::collections::HashSet<Entity> = labels.iter().map(|l| l.target).collect();
    for entity in &targets {
        if labeled.contains(&entity) {
            continue;
        }
        commands.spawn((
            Text2d::new(""),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.85, 1.0, 0.85)),
            // 中央下 anchor (= キャラ頭上の中央寄せ)。
            Anchor(Vec2::new(0.0, -0.5)),
            Transform::from_xyz(0.0, 0.0, 100.0),
            Visibility::Hidden,
            RenderLayers::layer(FINAL_PASS_LAYER),
            StateDebugLabel { target: entity },
        ));
    }
}

/// 各 `StateDebugLabel` の target から現状態を吸い出してテキスト化し、頭上に配置する。
/// target が despawn 済みなら label 自身も despawn する。
///
/// 位置は `FinalPassCamera` の world coord (= window 中心基準の px) に変換する:
///   scene world (= main camera centered) - main_camera.translation
///     → viewport-relative px
///   * viewport→window scale (= N * intermediate→window scale)
///     → window-pixel offset from window center
#[allow(clippy::type_complexity)]
fn update_labels(
    enabled: Res<StateDebugEnabled>,
    config: Option<Res<PixelPerfectConfig>>,
    mut commands: Commands,
    targets: Query<(
        &WorldPosition,
        &CharacterState,
        &AnimationFrames,
        &Combatant,
        &PhysicsParams,
        Option<&HitPoints>,
    )>,
    main_camera: Query<&Transform, (With<MainCamera>, Without<StateDebugLabel>)>,
    mut labels: Query<(
        Entity,
        &StateDebugLabel,
        &mut Text2d,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let viewport_to_window = config.as_deref().map_or(1.0, viewport_to_window_scale);
    let main_cam_pos = main_camera.single().map_or(Vec3::ZERO, |t| t.translation);
    for (label_entity, label, mut text, mut transform, mut vis) in &mut labels {
        let Ok((pos, state, anim, combatant, phys, hp)) = targets.get(label.target) else {
            // target は既に despawn 済み。label も回収する。
            commands.entity(label_entity).despawn();
            continue;
        };
        *vis = if enabled.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if !enabled.0 {
            continue;
        }
        text.0 = format_label(state, anim, combatant, phys, hp);
        // 頭上 (足元 +90 px) を scene world で計算 → main camera 基準にして viewport
        // 中心相対座標 → window scale を掛けて FinalPassCamera world 座標へ。
        let scene_world =
            projection::world_to_bevy_f32(pos.x.round(), pos.y.round() + 90.0, pos.z.round());
        let viewport_rel = scene_world - main_cam_pos;
        transform.translation = Vec3::new(
            viewport_rel.x * viewport_to_window,
            viewport_rel.y * viewport_to_window,
            100.0,
        );
    }
}

/// 1 viewport px = 何 window px か。`PixelPerfectConfig` から:
///   N = intermediate / viewport (整数)
///   linear_scale = min(window / intermediate, ...)
///   viewport→window = N * linear_scale
#[must_use]
fn viewport_to_window_scale(config: &PixelPerfectConfig) -> f32 {
    let viewport_w = config.viewport.0 as f32;
    let intermediate_w = config.intermediate.0 as f32;
    let n = intermediate_w / viewport_w;
    let linear_scale = (config.window.0 as f32 / intermediate_w)
        .min(config.window.1 as f32 / config.intermediate.1 as f32);
    n * linear_scale
}

/// 表示用 multiline 文字列を組み立てる。1 行あたり情報量を絞って 6 行に収める。
///
/// 行構成:
/// 1. `State(role)  t=<state 経過 tick>`
/// 2. `f=<frame 番号 (1-indexed)>/<frame count>  <frame tick 番号 (1-indexed)>/<frame 合計 tick>`
/// 3. `HP=<current>/<max>`
/// 4. `G=<gauge>/<threshold>` (Knockback ゲージ)
/// 5. `B=<remaining>/<max> FA=<final_action>` (Bounce 残数 + 終端 Action)
/// 6. `HFB=<true|false>` (Hit From Behind)
///
/// `f` と frame 内 tick は人間向けに 1-indexed で出し、frame count / frame 合計 tick と単位を
/// 揃える (最終 frame の最終 tick で `f=5/5  7/7` のように見える)。
/// `t=` (state 経過総 tick) は単独の積算値なので 0-indexed のまま。
/// 空 animation / duration 0 frame は分子 = 0 で出す (例外的に `0/0` を許容)。
fn format_label(
    state: &CharacterState,
    anim: &AnimationFrames,
    combatant: &Combatant,
    phys: &PhysicsParams,
    hp: Option<&HitPoints>,
) -> String {
    let role = state.to_role();
    let frame_count = anim.frame_count();
    let frame_human = if frame_count == 0 {
        0
    } else {
        anim.current_index() + 1
    };
    let frame_elapsed = anim.current_frame_elapsed_ticks();
    let frame_total = anim.current_frame_total_ticks();
    // frame 内 tick も 1-indexed: 「今この frame の N 番目の vsync を見ている」表現。
    // frame_total を上限としてクランプ (non-loop end の `elapsed == cur_dur` 状態でも
    // `7/7` で固定し、`8/7` にならないようにする)。
    let tick_human = if frame_count == 0 || frame_total == 0 {
        0
    } else {
        (frame_elapsed + 1).min(frame_total)
    };
    let state_elapsed = anim.total_elapsed_ticks();
    let hp_str = hp.map_or_else(|| "-".to_string(), |h| format!("{}/{}", h.current, h.max));
    let threshold = phys.0.knockback_threshold;
    let final_action = combatant.final_action;
    let hfb = combatant.hit_from_behind;
    let bounces = combatant.remaining_bounces;
    let max_bounces = phys.0.bounce_count;
    format!(
        "{state:?}({role:?}) t={state_elapsed}\nf={frame_human}/{frame_count} {tick_human}/{frame_total}\nHP={hp_str}\nG={}/{threshold}\nB={bounces}/{max_bounces} FA={final_action:?}\nHFB={hfb}",
        combatant.gauge,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::character::Physics;
    use crate::features::character::knockback::FinalAction;

    fn dummy_anim() -> AnimationFrames {
        AnimationFrames::new(vec![], false, 0)
    }

    fn dummy_combatant() -> Combatant {
        Combatant {
            gauge: 80,
            gauge_recovery_remaining_ticks: 0,
            stage_timer_ticks: 0,
            remaining_bounces: 2,
            final_action: FinalAction::LieDown,
            hit_from_behind: false,
            juggle_count: 0,
            down_hit_count: 0,
        }
    }

    fn dummy_physics() -> PhysicsParams {
        PhysicsParams(Physics {
            knockback_threshold: 100,
            bounce_count: 2,
            ..Physics::default()
        })
    }

    #[test]
    fn format_label_includes_all_fields() {
        let state = CharacterState::KnockbackUp;
        let anim = dummy_anim();
        let combatant = dummy_combatant();
        let phys = dummy_physics();
        let hp = HitPoints {
            current: 30,
            max: 60,
        };
        let s = format_label(&state, &anim, &combatant, &phys, Some(&hp));
        assert!(s.contains("KnockbackUp"), "state: {s}");
        assert!(s.contains("t=0"), "state elapsed: {s}");
        // empty animation → 0/0 (edge case allowed)
        assert!(s.contains("f=0/0"), "frame: {s}");
        assert!(s.contains("HP=30/60"), "hp: {s}");
        assert!(s.contains("G=80/100"), "gauge: {s}");
        assert!(s.contains("B=2/2"), "bounces: {s}");
        assert!(s.contains("FA=LieDown"), "final_action: {s}");
        assert!(s.contains("HFB=false"), "hit_from_behind: {s}");
    }

    fn dummy_frame_render(duration_ms: u64) -> crate::features::character::FrameRender {
        crate::features::character::FrameRender {
            handle: bevy::asset::Handle::default(),
            anchor: bevy::sprite::Anchor::default(),
            duration: std::time::Duration::from_millis(duration_ms),
            flip_x: false,
            alpha: 1.0,
            attack_meta: None,
            attack_box_geom: None,
            body_box_geoms: vec![],
            body_box_disabled: false,
            sprite_pivot: [0, 0],
        }
    }

    #[test]
    fn format_label_frame_index_is_one_indexed() {
        // 5 frames、`current_index()=0` → 表示は `f=1/5` (最終 frame で `f=5/5`)。
        let anim = AnimationFrames::new(vec![dummy_frame_render(100); 5], false, 0);
        let s = format_label(
            &CharacterState::Idle,
            &anim,
            &dummy_combatant(),
            &dummy_physics(),
            None,
        );
        assert!(s.contains("f=1/5"), "expected 1-indexed frame: {s}");
    }

    #[test]
    fn format_label_frame_tick_is_one_indexed_at_start() {
        // 構築直後 elapsed=0、frame_total = floor(100ms / 16.667ms) = 5 → 表示 `1/5`
        // (= 「現 frame の 1 tick 目を見ている」)。
        // 進行中・末尾クランプの挙動は format_label の `.min(frame_total)` の単純な
        // 算術で、`current_frame_elapsed_ticks` の挙動自体は animation.rs 側で別途
        // 単体テスト済み。
        let anim = AnimationFrames::new(vec![dummy_frame_render(100); 1], false, 0);
        let s = format_label(
            &CharacterState::Idle,
            &anim,
            &dummy_combatant(),
            &dummy_physics(),
            None,
        );
        assert!(s.contains("1/5"), "expected 1/5 at start: {s}");
    }

    #[test]
    fn viewport_to_window_scale_matches_n_times_linear_scale() {
        // viewport 384x216, intermediate 1152x648 (N=3), window 1280x720
        // → N=3, linear_scale=1280/1152≒1.111、min(1280/1152, 720/648) も同じ。
        // → viewport→window ≒ 3.333。
        let cfg = PixelPerfectConfig {
            viewport: (384, 216),
            intermediate: (1152, 648),
            window: (1280, 720),
        };
        let s = viewport_to_window_scale(&cfg);
        // 期待値: 3 * 1280/1152 ≒ 3.3333
        let expected = 3.0 * (1280.0_f32 / 1152.0);
        assert!((s - expected).abs() < 1e-4, "got {s}, expected {expected}");
    }

    #[test]
    fn viewport_to_window_scale_handles_letterbox_min() {
        // window が縦長 → 縦方向の比率が小さい → min 採用。
        let cfg = PixelPerfectConfig {
            viewport: (100, 100),
            intermediate: (200, 200),
            window: (1000, 400),
        };
        // N=2, linear_scale = min(1000/200, 400/200) = min(5, 2) = 2。
        // viewport→window = 2 * 2 = 4。
        let s = viewport_to_window_scale(&cfg);
        assert!((s - 4.0).abs() < 1e-4, "got {s}");
    }

    #[test]
    fn format_label_marks_dead_and_back_when_set() {
        let state = CharacterState::LieDown;
        let anim = dummy_anim();
        let mut combatant = dummy_combatant();
        combatant.final_action = FinalAction::Dead;
        combatant.hit_from_behind = true;
        combatant.gauge = -5;
        combatant.remaining_bounces = 0;
        let s = format_label(&state, &anim, &combatant, &dummy_physics(), None);
        assert!(s.contains("FA=Dead"));
        assert!(s.contains("HFB=true"));
        assert!(s.contains("G=-5/"));
        assert!(s.contains("HP=-"), "HP should show '-' when None: {s}");
    }
}
