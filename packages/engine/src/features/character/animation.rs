//! 連番スプライト・アニメーション (FSD: feature slice)。
//!
//! `AnimationFrames` コンポーネントを attach した entity の `Sprite.image` を、
//! frame ごとの `duration` で進める。`is_loop` と `loop_start_index` で
//! ループの巻き戻し先を制御できる (animations YAML の挙動と整合)。
//!
//! 各 frame は (handle, anchor, duration, flip_x, alpha) を [`FrameRender`] で保持する。
//! `flip_x` は animation YAML の `frame.flip` と `layer.flip` を合成済み (XOR) の値。
//! 最終的な sprite.flip_x は [`super::movement::sync_flip`] が Facing と XOR する。
//! Anchor 更新は [`super::movement::sync_anchor`] が担当する。
//! Alpha (transparency) は本モジュールの `sync_transparency` が反映する。
use std::time::Duration;

use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::sprite::Anchor;

/// 1 frame ぶんの描画パラメータ。
#[derive(Debug, Clone)]
pub struct FrameRender {
    pub handle: Handle<Image>,
    pub anchor: Anchor,
    pub duration: Duration,
    /// frame.flip XOR layer.flip の合成済み x 反転フラグ (Facing は別途合成)。
    pub flip_x: bool,
    /// 0.0..=1.0 の透明度。`Sprite.color` の alpha に焼く。
    pub alpha: f32,
}

#[derive(Component)]
pub struct AnimationFrames {
    frames: Vec<FrameRender>,
    is_loop: bool,
    loop_start_index: usize,
    current: usize,
    timer: Timer,
}

impl AnimationFrames {
    /// 空 frames で構築されると `tick` は何もしない (`spawn` 時の defensive default 用)。
    #[must_use]
    pub fn new(frames: Vec<FrameRender>, is_loop: bool, loop_start_index: usize) -> Self {
        let first = frames
            .first()
            .map_or(Duration::from_millis(100), |f| f.duration);
        Self {
            frames,
            is_loop,
            loop_start_index,
            current: 0,
            timer: Timer::new(first, TimerMode::Once),
        }
    }

    /// 画像 dimensions と pivot 位置 (画像ピクセル) から Bevy の Anchor を計算する。
    /// `anchor_x = (pivot_x / width) - 0.5`、`anchor_y = 0.5 - (pivot_y / height)` で
    /// 画像 (pivot_x, pivot_y) が Transform.translation に来るようにする (Y は反転)。
    #[must_use]
    pub fn anchor_from_pivot(
        image_width: u32,
        image_height: u32,
        pivot_x: i32,
        pivot_y: i32,
    ) -> Anchor {
        let w = image_width.max(1) as f32;
        let h = image_height.max(1) as f32;
        Anchor(Vec2::new(pivot_x as f32 / w - 0.5, 0.5 - pivot_y as f32 / h))
    }

    #[must_use]
    pub fn current_anchor(&self) -> Anchor {
        self.frames[self.current].anchor
    }

    #[must_use]
    pub fn current_flip_x(&self) -> bool {
        self.frames[self.current].flip_x
    }

    #[must_use]
    pub fn current_alpha(&self) -> f32 {
        self.frames[self.current].alpha
    }

    /// 次フレームの index を決める。ループ末尾で `loop_start_index` に巻き戻す。
    fn next_index(&self) -> usize {
        if self.current + 1 < self.frames.len() {
            self.current + 1
        } else if self.is_loop {
            self.loop_start_index.min(self.frames.len() - 1)
        } else {
            self.current
        }
    }
}

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (tick, sync_transparency));
    }
}

fn tick(time: Res<Time>, mut query: Query<(&mut Sprite, &mut AnimationFrames)>) {
    for (mut sprite, mut anim) in &mut query {
        if anim.frames.is_empty() {
            continue;
        }
        anim.timer.tick(time.delta());
        if !anim.timer.just_finished() {
            continue;
        }
        let next = anim.next_index();
        if next == anim.current {
            continue;
        }
        anim.current = next;
        sprite.image = anim.frames[anim.current].handle.clone();
        let dur = anim.frames[anim.current].duration;
        anim.timer = Timer::new(dur, TimerMode::Once);
    }
}

fn sync_transparency(mut query: Query<(&AnimationFrames, &mut Sprite), Changed<AnimationFrames>>) {
    for (anim, mut sprite) in &mut query {
        let alpha = anim.current_alpha();
        sprite.color = sprite.color.with_alpha(alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_from_pivot_centered_pivot_yields_center_anchor() {
        let a = AnimationFrames::anchor_from_pivot(40, 100, 20, 50);
        assert!((a.0.x - 0.0).abs() < f32::EPSILON);
        assert!((a.0.y - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn anchor_from_pivot_bottom_center_matches_bottom_center_constant() {
        let a = AnimationFrames::anchor_from_pivot(40, 100, 20, 100);
        assert!((a.0.x - 0.0).abs() < f32::EPSILON);
        assert!((a.0.y - (-0.5)).abs() < f32::EPSILON);
    }

    #[test]
    fn anchor_from_pivot_walk_frame_001() {
        let a = AnimationFrames::anchor_from_pivot(37, 95, 23, 93);
        assert!((a.0.x - (23.0_f32 / 37.0 - 0.5)).abs() < f32::EPSILON);
        assert!((a.0.y - (0.5 - 93.0_f32 / 95.0)).abs() < f32::EPSILON);
    }

    fn dummy_frame(ms: u64) -> FrameRender {
        FrameRender {
            handle: Handle::<Image>::default(),
            anchor: Anchor::default(),
            duration: Duration::from_millis(ms),
            flip_x: false,
            alpha: 1.0,
        }
    }

    #[test]
    fn next_index_advances_within_range() {
        let frames = AnimationFrames::new(vec![dummy_frame(100); 3], true, 0);
        assert_eq!(frames.next_index(), 1);
    }

    #[test]
    fn next_index_loops_to_loop_start() {
        let mut frames = AnimationFrames::new(vec![dummy_frame(100); 3], true, 1);
        frames.current = 2;
        assert_eq!(frames.next_index(), 1);
    }

    #[test]
    fn next_index_stops_at_end_when_not_loop() {
        let mut frames = AnimationFrames::new(vec![dummy_frame(100); 3], false, 0);
        frames.current = 2;
        assert_eq!(frames.next_index(), 2);
    }

    #[test]
    fn current_flip_x_and_alpha_reflect_frame() {
        let mut f = dummy_frame(100);
        f.flip_x = true;
        f.alpha = 0.5;
        let frames = AnimationFrames::new(vec![f], false, 0);
        assert!(frames.current_flip_x());
        assert!((frames.current_alpha() - 0.5).abs() < f32::EPSILON);
    }
}
