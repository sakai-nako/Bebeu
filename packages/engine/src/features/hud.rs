//! Gameplay HUD slice (ADR-0029)。
//!
//! `Project.hud.elements` を読み、battle scene 入場時に screen-anchored な Sprite として
//! HUD 要素を spawn する。MainCamera の子として attach することで camera の X 追従に
//! 同期させ、別 system は要らない。
//!
//! 現状の要素: `PlayerHpBar` のみ。要素を増やすときは [`HudElement`] に variant と
//! 対応する Config struct を生やし、spawn_hud の match と更新 system を 1 ペア追加する。
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::app::SceneState;
use crate::entities::project::{
    FillDirection, GaugeStep, HudAnchor, HudElement, HudOffset, PlayerHpBarConfig, Project,
};
use crate::features::character::{HitPoints, MainCamera, Player};

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
                update_player_hp_bar.run_if(in_state(SceneState::Battle)),
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
    hp_low: f32,
    hp_high: f32,
    full_size: Vec2,
    fill_direction: FillDirection,
}

fn spawn_hud(
    mut commands: Commands,
    project: Option<Res<Project>>,
    camera_query: Query<Entity, With<MainCamera>>,
    player_query: Query<&HitPoints, With<Player>>,
    existing: Query<(), With<HudRoot>>,
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
    // Player の max HP は gauge segment の HP 範囲を決めるのに必要。Player が居ない
    // smoke test では Some にならず spawn を skip する。
    let Ok(hp) = player_query.single() else {
        return;
    };

    let viewport = (
        project.resolution.width as f32,
        project.resolution.height as f32,
    );
    let max_hp = hp.max as f32;

    for element in &project.hud.elements {
        let top_left = top_left_of_element(element.anchor(), element.offset(), viewport);
        match element {
            HudElement::PlayerHpBar(cfg) => {
                spawn_player_hp_bar(&mut commands, camera, top_left, *cfg, max_hp);
            }
        }
    }
}

fn despawn_hud(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn update_player_hp_bar(
    player_query: Query<&HitPoints, With<Player>>,
    mut gauges: Query<(&mut Sprite, &PlayerHpBarGauge)>,
) {
    let Ok(hp) = player_query.single() else {
        return;
    };
    let current = hp.current as f32;
    for (mut sprite, gauge) in &mut gauges {
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
    camera: Entity,
    top_left: Vec2,
    cfg: PlayerHpBarConfig,
    max_hp: f32,
) {
    let outer_size = Vec2::new(cfg.size.w, cfg.size.h);
    let frame_t = cfg.frame.thickness.max(0.0);
    let inner_size = Vec2::new(
        (cfg.size.w - 2.0 * frame_t).max(0.0),
        (cfg.size.h - 2.0 * frame_t).max(0.0),
    );

    let root_translation = Vec3::new(top_left.x, top_left.y, HUD_Z);
    let root = commands
        .spawn((
            HudRoot,
            Transform::from_translation(root_translation),
            Visibility::default(),
            ChildOf(camera),
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
                hp_low: seg.hp_low,
                hp_high: seg.hp_high,
                full_size: seg.full_size,
                fill_direction: cfg.fill_direction,
            },
            ChildOf(root),
        ));
    }
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
}
