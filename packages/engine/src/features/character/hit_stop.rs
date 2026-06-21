//! Hit stop 演出 (FSD: feature slice)。
//!
//! Attack が hit したとき、attacker と victim に [`HitStopState`] component が attach
//! される (= [`super::attack::resolve_hits`] の責務)。本 module の system は:
//!
//! - 毎 frame `remaining_ms` を減算し、0 になったら component を remove
//! - victim 側は Sprite の `Transform.translation` に shake (三角波の片道 count ぶん) を
//!   `(1 - decay * progress)` で線形減衰させた振幅で乗せる (= 旧 impact + shake を統合)
//!   (world position は不動、見た目だけ揺らす)
//!
//! 加えて [`super::animation::AnimationFrames`] の進行と
//! [`super::state_machine::end_oneshot_actions`] の state 遷移は `Without<HitStopState>`
//! filter で hit_stop 中だけ skip される (= time freeze)。
//!
//! 軸:
//! - X: キャラ向きの前方が + (Facing で符号反転)
//! - Y: 画面上が + (= Bevy Y と一致)
use bevy::prelude::*;

use crate::shared::projection;

use super::debug_control::SimulationSet;
use super::movement::{Facing, WorldPosition};

/// hit_stop 中の entity に attach される component。`remaining_ms` が 0 を切ったら
/// remove され、Animation 進行 / state 遷移が再開される。
#[derive(Component, Debug, Clone, Copy)]
pub struct HitStopState {
    /// hit_stop の総時間 (ms)。shake の進行度 (`progress = 1 - remaining/total`) 計算に使う。
    pub total_ms: u32,
    /// 残り時間 (ms)。`f32` で持って sub-frame 精度を保つ。
    pub remaining_ms: f32,
    /// shake の初期振幅。中心 0 から ±shake_x。+ = キャラ向き前方。
    pub shake_x: i32,
    /// shake の初期振幅。中心 0 から ±shake_y。+ = 画面上。
    pub shake_y: i32,
    /// 片道回数 (= 中心 ↔ ±max の移動を 1 と数える)。1 = 中心 → +max で終了、
    /// 4 = 1 周期 (中心 → +max → 中心 → -max → 中心)。0 で shake なし。
    pub count: u32,
    /// shake 振幅の線形減衰率。`amplitude(progress) = shake * (1 - decay * progress).clamp(0, 1)`。
    /// 0.0 で振幅一定、1.0 で末尾の振幅 0。負値や 1 超は clamp で吸収。
    pub decay: f32,
}

impl HitStopState {
    /// attacker 用 (= time freeze だけ、shake 無し)。
    #[must_use]
    pub fn attacker(duration_ms: u32) -> Self {
        Self {
            total_ms: duration_ms,
            remaining_ms: duration_ms as f32,
            shake_x: 0,
            shake_y: 0,
            count: 0,
            decay: 0.0,
        }
    }

    /// victim 用 (= time freeze + shake (片道 `count` 回、振幅 `decay` 減衰))。
    /// 1 片道目はキャラ向き前方 (X) / 画面上 (Y) に振れる (= 旧 impact 相当)。
    #[must_use]
    pub fn victim(duration_ms: u32, shake_x: i32, shake_y: i32, count: u32, decay: f32) -> Self {
        Self {
            total_ms: duration_ms,
            remaining_ms: duration_ms as f32,
            shake_x,
            shake_y,
            count,
            decay,
        }
    }
}

pub struct HitStopPlugin;

impl Plugin for HitStopPlugin {
    fn build(&self, app: &mut App) {
        // movement::sync_transform より後に走らせる必要があるので PostUpdate。
        app.add_systems(PostUpdate, update_hit_stop.in_set(SimulationSet::Active));
    }
}

/// hit_stop の (1) 残り時間の減算と remove、(2) visual offset 適用を 1 system で扱う。
/// 順序問題 (tick と visual_offset の前後関係) を回避するため統合してある。
fn update_hit_stop(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut HitStopState,
        &Facing,
        &WorldPosition,
        &mut Transform,
    )>,
) {
    let dt_ms = time.delta_secs() * 1000.0;
    for (entity, mut state, facing, pos, mut transform) in &mut query {
        // base = projection で world position から計算した Bevy 座標 (offset 適用前の素の位置)。
        // movement::sync_transform は Changed<WorldPosition> filter なので hit_stop 中は
        // 走らないことが多い。base を毎 frame 再計算して上書きすることで、shake の各 frame で
        // 「素の位置 + 動的 offset」を保つ。
        // movement::sync_transform と同じく nearest filter snap を避けるため整数 snap。
        // shake offset 自体は演出として元から整数 px (shake_x/shake_y は u32) なので
        // base を整数化しておけば最終 translation も整数になる。
        let base = projection::world_to_bevy_f32(pos.x.round(), pos.y.round(), pos.z.round());

        let dir_x: f32 = match facing {
            Facing::Right => 1.0,
            Facing::Left => -1.0,
        };
        let (offset_x, offset_y) = if state.count > 0 {
            // shake: progress (0→1) で count 片道ぶんの三角波を回す。1 周期 = 4 片道なので
            // phase = progress * count / 4。 amplitude は (1 - decay * progress) で線形減衰。
            let progress = (1.0 - state.remaining_ms / state.total_ms as f32).clamp(0.0, 1.0);
            let phase = progress * (state.count as f32) * 0.25;
            let wave = triangle_wave(phase);
            let amp_factor = (1.0 - state.decay * progress).clamp(0.0, 1.0);
            (
                wave * (state.shake_x as f32) * amp_factor * dir_x,
                wave * (state.shake_y as f32) * amp_factor,
            )
        } else {
            (0.0, 0.0)
        };
        transform.translation = base + Vec3::new(offset_x, offset_y, 0.0);

        // 残り時間を減算し、0 を切ったら component を remove + Transform を base に戻す。
        state.remaining_ms -= dt_ms;
        if state.remaining_ms <= 0.0 {
            transform.translation = base;
            commands.entity(entity).remove::<HitStopState>();
        }
    }
}

/// 三角波 (周期 1)。`x=0` で `0`、`x=0.25` で `+1`、`x=0.5` で `0`、`x=0.75` で `-1`、
/// `x=1.0` で `0` に戻る。`x` の整数部はループ周期を表す (= 周期数)。
#[must_use]
fn triangle_wave(x: f32) -> f32 {
    let frac = x - x.floor();
    if frac < 0.25 {
        frac * 4.0
    } else if frac < 0.75 {
        2.0 - frac * 4.0
    } else {
        frac * 4.0 - 4.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_wave_quarters() {
        assert!((triangle_wave(0.0) - 0.0).abs() < 1e-6);
        assert!((triangle_wave(0.25) - 1.0).abs() < 1e-6);
        assert!((triangle_wave(0.5) - 0.0).abs() < 1e-6);
        assert!((triangle_wave(0.75) - (-1.0)).abs() < 1e-6);
        assert!((triangle_wave(1.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn triangle_wave_periods() {
        // 周期 1 = 整数部はループ。x=2.25 ≡ x=0.25 = 1.0
        assert!((triangle_wave(2.25) - 1.0).abs() < 1e-6);
        // x=3.5 ≡ x=0.5 = 0.0
        assert!((triangle_wave(3.5) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn attacker_constructor_disables_shake() {
        let s = HitStopState::attacker(100);
        assert_eq!(s.total_ms, 100);
        assert!((s.remaining_ms - 100.0).abs() < 1e-6);
        assert_eq!(s.shake_x, 0);
        assert_eq!(s.shake_y, 0);
        assert_eq!(s.count, 0);
        assert!((s.decay - 0.0).abs() < 1e-6);
    }

    #[test]
    fn victim_constructor_carries_shake_count_and_decay() {
        let s = HitStopState::victim(120, 2, 4, 3, 0.5);
        assert_eq!(s.total_ms, 120);
        assert!((s.remaining_ms - 120.0).abs() < 1e-6);
        assert_eq!(s.shake_x, 2);
        assert_eq!(s.shake_y, 4);
        assert_eq!(s.count, 3);
        assert!((s.decay - 0.5).abs() < 1e-6);
    }
}
