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

use crate::entities::character::{AttackBoxMeta, HitBox, HitStop};

use super::hit_stop::HitStopState;

/// 60Hz vsync の 1 tick の長さ (= 1/60 秒 ≈ 16.667ms)。Animation の `tick` system は
/// Update に居て 1 vsync で 1 回呼ばれる前提で、`time.delta()` の jitter を無視して
/// この固定値を accumulator に加算する。
/// データモデル側 (`Frame.ticks`) が 60Hz tick 数を 1 級概念として持つので、
/// engine は `ticks * VSYNC_TICK` で `Duration` 化するだけでよい。
///
/// 高 refresh display (120Hz 等) では animation 速度がズレるので、いずれ
/// display rate を検出して動的に決める拡張は要検討。
pub const VSYNC_TICK: Duration = Duration::from_micros(16_667);
/// `VSYNC_TICK` の秒換算 (= 1/60)。movement 等の f32 計算で使う。
pub const VSYNC_TICK_SECS: f32 = 1.0 / 60.0;

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
    /// この frame で active な AttackBox の効果データ (damage / knockback_damage /
    /// knockback ベクトル / hit_stop)。`None` で attack 判定なし。
    /// `Frame.attack_box_overrides` と sprite 側 `attack_boxes` を merge した結果
    /// (= `resolve_attack_box`) を build 時に焼き込む。`current_attack_damage` /
    /// `current_attack_hit_stop` 等の旧 API は本 field から派生する。
    pub attack_meta: Option<AttackBoxMeta>,
    /// この frame で active な AttackBox の幾何 (画像 pixel 内 HitBox)。
    /// `Some` のとき `world_box_from_hitbox` でこの frame の `sprite_pivot` を基準に world XYZ
    /// box を求める。`None` なら attack 判定なし (`attack_meta` も None 想定)。
    pub attack_box_geom: Option<HitBox>,
    /// この frame で active な BodyBox の幾何 (画像 pixel 内 HitBox)。Vec で複数 box を
    /// 許容するが、engine が見るのは現状 **先頭要素だけ**。空なら BodyBox なし。
    /// Frame.body_box_overrides の 3-state (Inherit/Disable/Override) と SpriteEntry.body_boxes
    /// を解釈した結果を build 時に焼き込む。
    pub body_box_geoms: Vec<HitBox>,
    /// 現フレームの最終 pivot 位置 (画像 pixel)。`SpriteEntry.pivot_point` に
    /// `frame.pivot_point_offset` と `layer.pivot_point_offset` を加算したもの。
    /// AttackBox / BodyBox の世界変換で「画像座標 → world 座標」の原点として使う。
    pub sprite_pivot: [i32; 2],
}

#[derive(Component)]
pub struct AnimationFrames {
    frames: Vec<FrameRender>,
    is_loop: bool,
    loop_start_index: usize,
    current: usize,
    /// 現 frame に費やした累積時間。`tick` で `time.delta()` を加算し、`current` frame の
    /// `duration` を超えたら次 frame へ進めると同時に超過分をそのまま持ち越す。
    /// `Timer` を毎 frame `Timer::new()` で作り直すと超過分が捨てられ、120ms 等
    /// vsync (≒16.67ms) の整数倍でない duration で jitter が出るため、accumulator 方式にする。
    elapsed_in_frame: Duration,
}

impl AnimationFrames {
    /// 空 frames で構築されると `tick` は何もしない (`spawn` 時の defensive default 用)。
    #[must_use]
    pub fn new(frames: Vec<FrameRender>, is_loop: bool, loop_start_index: usize) -> Self {
        Self {
            frames,
            is_loop,
            loop_start_index,
            current: 0,
            elapsed_in_frame: Duration::ZERO,
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
        Anchor(Vec2::new(
            pivot_x as f32 / w - 0.5,
            0.5 - pivot_y as f32 / h,
        ))
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

    /// 現在再生中の frame index (0-indexed)。hit frame 判定など、
    /// 「特定 frame でだけ何かをする」用途に使う。
    #[must_use]
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// 現在 frame の AttackBoxMeta (`None` なら攻撃判定なし)。
    /// attack 系 system はこの値で「今この frame で AttackBox を生やすか」を判定し、
    /// hit 解決時の damage / knockback / hit_stop もここから引く。
    #[must_use]
    pub fn current_attack_meta(&self) -> Option<&AttackBoxMeta> {
        self.frames
            .get(self.current)
            .and_then(|f| f.attack_meta.as_ref())
    }

    /// 現在 frame の damage 量 (`None` なら攻撃判定なし)。`current_attack_meta` の派生。
    #[must_use]
    pub fn current_attack_damage(&self) -> Option<u32> {
        self.current_attack_meta().map(|m| m.damage)
    }

    /// 現在 frame の AttackBox 幾何 (画像 pixel 内 HitBox)。
    #[must_use]
    pub fn current_attack_box_geom(&self) -> Option<&HitBox> {
        self.frames
            .get(self.current)
            .and_then(|f| f.attack_box_geom.as_ref())
    }

    /// 現在 frame の BodyBox 幾何 (画像 pixel 内 HitBox 列)。空なら BodyBox なし。
    #[must_use]
    pub fn current_body_boxes(&self) -> &[HitBox] {
        self.frames
            .get(self.current)
            .map_or(&[][..], |f| &f.body_box_geoms[..])
    }

    /// 現在 frame の最終 pivot (画像 pixel)。frames が空のときは `[0, 0]`。
    #[must_use]
    pub fn current_sprite_pivot(&self) -> [i32; 2] {
        self.frames
            .get(self.current)
            .map_or([0, 0], |f| f.sprite_pivot)
    }

    /// 現在 frame で active な hit_stop 演出パラメータ (`None` で hit_stop なし)。
    /// attack 系 system はヒット解決時にこれを参照して duration と impact/shake を決める。
    /// `current_attack_meta` の派生。
    #[must_use]
    pub fn current_attack_hit_stop(&self) -> Option<HitStop> {
        self.current_attack_meta().and_then(|m| m.hit_stop)
    }

    /// `frames[0].duration` (ms)。被弾側の Hit アニメ frame 0 を「のけぞり pose」と捉え、
    /// hit_stop.duration_ms が未指定のときの fallback duration として使う。空 frames では None。
    #[must_use]
    pub fn first_frame_duration_ms(&self) -> Option<u32> {
        self.frames
            .first()
            .map(|f| u32::try_from(f.duration.as_millis()).unwrap_or(u32::MAX))
    }

    /// is_loop=false で最終 frame の duration を消化済みなら true。
    /// ループ animation や空 frames では常に false を返す。
    #[must_use]
    pub fn is_finished(&self) -> bool {
        if self.is_loop || self.frames.is_empty() {
            return false;
        }
        let last = self.frames.len() - 1;
        self.current == last && self.elapsed_in_frame >= self.frames[last].duration
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

    /// `elapsed_in_frame` に `delta` を加算し、必要なら frame を進める。frame が
    /// 1 つ以上進んだら `true` を返す (sprite.image 差し替えのトリガに使う)。
    /// 超過分は次 frame に持ち越し、`delta` が大きく複数 frame ぶんを跨ぐ場合も
    /// while で連続的に消化する。
    fn advance(&mut self, delta: Duration) -> bool {
        if self.frames.is_empty() {
            return false;
        }
        self.elapsed_in_frame += delta;
        let mut advanced = false;
        loop {
            let cur_dur = self.frames[self.current].duration;
            if self.elapsed_in_frame < cur_dur {
                break;
            }
            let next = self.next_index();
            if next == self.current {
                // 非ループ末尾 or 単一 frame — これ以上進めないので duration で clamp。
                self.elapsed_in_frame = cur_dur;
                break;
            }
            self.elapsed_in_frame -= cur_dur;
            self.current = next;
            advanced = true;
        }
        advanced
    }
}

/// Animation 関連 system の実行フェーズ識別 (Bevy `SystemSet`)。
///
/// `AnimationSet::Tick` の **後に走る必要のある system**:
/// - [`super::movement::sync_anchor`] / [`super::movement::sync_flip`]
///   (`Changed<AnimationFrames>` で frame 切替時の anchor / flip を更新)
/// - [`super::attack::sync_body_box`] (現 frame の body_box 形状で box を再計算)
///
/// これらの順序を明示しないと、Bevy scheduler が `tick` より前に sync_* を走らせる
/// Update があり得て、**新 frame の `sprite.image` + 旧 frame の `Anchor` という
/// 1 frame だけのミスマッチ描画**が出る (= frame 切替の瞬間にカクッと見える症状)。
/// 各 plugin で `.after(AnimationSet::Tick)` を宣言する。
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnimationSet {
    /// `tick` system が `anim.current` を進めて `sprite.image` を swap するフェーズ。
    Tick,
}

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (tick.in_set(AnimationSet::Tick), sync_transparency));
    }
}

/// AnimationFrames の累積時間を進めて frame を切り替える。`HitStopState` が attach
/// されている entity は hit_stop 中なので skip する (= time freeze)。
///
/// `time.delta()` ではなく **固定値 [`VSYNC_TICK`]** を加算する。Update は 1 vsync
/// 1 回呼ばれる前提なので、これで「Update 1 回 = vsync 1 tick」が成立し、
/// `Frame.ticks * VSYNC_TICK` で組み立てた duration と組み合わせると pose 切替が
/// 完全に均等な tick 数で発火する。`time.delta()` の jitter (DWM compositor の
/// 微小な vsync ズレ) を全部無視できる。
fn tick(mut query: Query<(&mut Sprite, &mut AnimationFrames), Without<HitStopState>>) {
    for (mut sprite, mut anim) in &mut query {
        if anim.advance(VSYNC_TICK) {
            sprite.image = anim.frames[anim.current].handle.clone();
        }
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
            attack_meta: None,
            attack_box_geom: None,
            body_box_geoms: Vec::new(),
            sprite_pivot: [0, 0],
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
    fn is_finished_false_for_loop() {
        let frames = AnimationFrames::new(vec![dummy_frame(100); 2], true, 0);
        assert!(!frames.is_finished());
    }

    #[test]
    fn is_finished_false_before_last_frame_completes() {
        // 末尾 frame に居ても timer 未消化なら未完了。
        let mut frames = AnimationFrames::new(vec![dummy_frame(100); 2], false, 0);
        frames.current = 1;
        assert!(!frames.is_finished());
    }

    #[test]
    fn is_finished_true_when_last_frame_timer_done() {
        let mut frames = AnimationFrames::new(vec![dummy_frame(100); 2], false, 0);
        frames.current = 1;
        frames.elapsed_in_frame = Duration::from_millis(200);
        assert!(frames.is_finished());
    }

    #[test]
    fn is_finished_false_for_empty_frames() {
        let frames = AnimationFrames::new(vec![], false, 0);
        assert!(!frames.is_finished());
    }

    #[test]
    fn current_index_starts_at_zero_and_reflects_advance() {
        let mut frames = AnimationFrames::new(vec![dummy_frame(100); 3], true, 0);
        assert_eq!(frames.current_index(), 0);
        frames.current = 2;
        assert_eq!(frames.current_index(), 2);
    }

    #[test]
    fn current_attack_damage_returns_frame_value() {
        let mut f = dummy_frame(100);
        f.attack_meta = Some(AttackBoxMeta {
            damage: 40,
            ..AttackBoxMeta::default()
        });
        let frames = AnimationFrames::new(vec![f, dummy_frame(100)], false, 0);
        assert_eq!(frames.current_attack_damage(), Some(40));
    }

    #[test]
    fn current_attack_damage_none_when_unset() {
        let frames = AnimationFrames::new(vec![dummy_frame(100); 2], false, 0);
        assert_eq!(frames.current_attack_damage(), None);
    }

    #[test]
    fn current_attack_meta_returns_full_meta() {
        let mut f = dummy_frame(100);
        f.attack_meta = Some(AttackBoxMeta {
            damage: 20,
            knockback_damage: 30,
            ..AttackBoxMeta::default()
        });
        let frames = AnimationFrames::new(vec![f], false, 0);
        let meta = frames.current_attack_meta().expect("should be set");
        assert_eq!(meta.damage, 20);
        assert_eq!(meta.knockback_damage, 30);
    }

    #[test]
    fn current_attack_box_geom_returns_frame_value() {
        let mut f = dummy_frame(100);
        f.attack_box_geom = Some(HitBox {
            top_left: [0, 0],
            bottom_right: [10, 10],
            depth: None,
        });
        let frames = AnimationFrames::new(vec![f, dummy_frame(100)], false, 0);
        let geom = frames.current_attack_box_geom().expect("should be set");
        assert_eq!(geom.bottom_right, [10, 10]);
    }

    #[test]
    fn current_sprite_pivot_falls_back_to_zero_when_empty() {
        let frames = AnimationFrames::new(vec![], false, 0);
        assert_eq!(frames.current_sprite_pivot(), [0, 0]);
    }

    #[test]
    fn current_body_boxes_returns_frame_value() {
        let mut f = dummy_frame(100);
        f.body_box_geoms = vec![HitBox {
            top_left: [14, 18],
            bottom_right: [34, 60],
            depth: Some(16),
        }];
        let frames = AnimationFrames::new(vec![f, dummy_frame(100)], false, 0);
        let bodies = frames.current_body_boxes();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0].bottom_right, [34, 60]);
    }

    #[test]
    fn current_body_boxes_empty_when_unset() {
        let frames = AnimationFrames::new(vec![dummy_frame(100)], false, 0);
        assert!(frames.current_body_boxes().is_empty());
    }

    #[test]
    fn current_sprite_pivot_returns_frame_value() {
        let mut f = dummy_frame(100);
        f.sprite_pivot = [24, 90];
        let frames = AnimationFrames::new(vec![f], false, 0);
        assert_eq!(frames.current_sprite_pivot(), [24, 90]);
    }

    #[test]
    fn advance_carries_overshoot_into_next_frame() {
        // 100ms × 3 frames, delta=130ms → 1 frame 進んで 30ms ぶん次 frame を消化済み。
        let mut frames = AnimationFrames::new(vec![dummy_frame(100); 3], true, 0);
        assert!(frames.advance(Duration::from_millis(130)));
        assert_eq!(frames.current, 1);
        assert_eq!(frames.elapsed_in_frame, Duration::from_millis(30));
    }

    #[test]
    fn advance_consumes_multiple_frames_when_delta_large() {
        // 100ms × 4 frames, delta=250ms → 2 frame 進んで 50ms 残り。
        let mut frames = AnimationFrames::new(vec![dummy_frame(100); 4], true, 0);
        assert!(frames.advance(Duration::from_millis(250)));
        assert_eq!(frames.current, 2);
        assert_eq!(frames.elapsed_in_frame, Duration::from_millis(50));
    }

    #[test]
    fn advance_average_frame_time_converges_to_duration_under_jitter() {
        // vsync (16.67ms) と 120ms の整数倍ズレで jitter 出ないことを確認: 6 frame ぶんの
        // delta 合計 (720ms) を 16.67ms 刻みで投入したとき、ちょうど 1 周 (current が
        // loop_start_index に戻る) しているはず。`Timer::new` 方式だと毎 frame 超過分
        // (13.33ms) が捨てられ、6 frame で 80ms 遅れて 6 周未満になる回帰を防ぐ。
        let mut frames = AnimationFrames::new(vec![dummy_frame(120); 6], true, 0);
        // 16.67ms ≒ 16667μs。720_000μs / 16667μs ≒ 43.2 tick。43 tick + 余り。
        let total_us: u64 = 6 * 120_000;
        let tick_us: u64 = 16_667;
        let mut consumed: u64 = 0;
        while consumed + tick_us <= total_us {
            frames.advance(Duration::from_micros(tick_us));
            consumed += tick_us;
        }
        // 残り (≒3μs〜) を投入して合計 720ms に到達させる。
        frames.advance(Duration::from_micros(total_us - consumed));
        // 6 frame ぴったり消化したので current=0 に戻り、elapsed_in_frame ≒ 0。
        assert_eq!(frames.current, 0);
        assert!(frames.elapsed_in_frame < Duration::from_millis(1));
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
