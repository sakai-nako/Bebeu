//! Animation 再生のステートとタイマースレッド管理。
//!
//! - `PlaybackState` で再生・一時停止・停止を表現する 3 状態
//! - `use_playback_provider` を `AnimationEditor` で 1 回呼び、`use_playback` で子から消費
//!   (`Preferences` / `NavigationGuard` と同じ context-shared Signal の慣習)
//! - `spawn_playback_thread` は `std::thread::spawn` ベースのタイマー。
//!   `Frame.ticks` 個ぶん vsync 待ち (= `ticks * 1/60 秒` sleep) して次のフレームを
//!   `pending_frame` に書き込む
//!
//! ## Sync Signal とブリッジ
//!
//! Dioxus の default Signal は `UnsyncStorage` (RefCell) で `!Send` のため、`std::thread::spawn`
//! の closure 境界を超えられない。再生スレッドが触るものだけ `SyncStorage` に切り替えて、
//! UI 側の通常 (Unsync) Signal とは `use_effect` でブリッジする:
//!
//! - `Signal<PlaybackState, SyncStorage>` … スレッドが終了時に `Stopped` を書き戻す
//! - `Signal<Option<usize>, SyncStorage>` (pending_frame) … スレッドが次フレーム index を書く。
//!   UI 側の `use_effect` が読み取って `selected_frame_index` (Unsync) に転送し、`None` にリセット
//! - `Signal<PlaybackConfig, SyncStorage>` … 再生に必要な Animation 情報のスナップショット。
//!   UI 側の `use_effect` が `draft` 変更を監視してミラーする
//!
//! cancel_token (Arc<AtomicBool>) は呼び出し側 (Timeline) が `new_cancel_token()` で生成して保持し、
//! Pause/Stop または unmount 時に true を立てるとスレッドが次の chunk 境界で抜ける。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dioxus::prelude::*;

use super::Animation;

/// 再生状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

impl PlaybackState {
    #[must_use]
    pub fn is_playing(self) -> bool {
        matches!(self, Self::Playing)
    }

    /// 編集 UI をロックすべきか。Pause 中は編集可能にする方針なので Playing のみ true。
    #[must_use]
    pub fn locks_editing(self) -> bool {
        matches!(self, Self::Playing)
    }
}

/// 再生スレッドが参照する Animation のスナップショット。
/// Send + Sync で thread に渡せる。UI 側で `draft` 変更時に再構築する。
#[derive(Debug, Clone, Default)]
pub struct PlaybackConfig {
    /// 各 frame の寿命 (60Hz vsync tick 数)。エンジンの `Frame.ticks` と同じ単位。
    pub frame_ticks: Vec<u32>,
    pub is_loop: bool,
    pub loop_start_index: u32,
}

impl PlaybackConfig {
    #[must_use]
    pub fn from_animation(anim: &Animation) -> Self {
        Self {
            frame_ticks: anim.frames.iter().map(|f| f.ticks).collect(),
            is_loop: anim.is_loop,
            loop_start_index: anim.loop_start_index,
        }
    }
}

/// `AnimationEditor` で context に PlaybackState の Signal を提供する (SyncStorage)。
pub fn use_playback_provider() -> Signal<PlaybackState, SyncStorage> {
    use_context_provider(|| Signal::new_maybe_sync(PlaybackState::Stopped))
}

/// 子コンポーネントから PlaybackState の Signal を取得する。
/// provider なしで呼ぶと panic するので、必ず `AnimationEditor` 配下で使うこと。
pub fn use_playback() -> Signal<PlaybackState, SyncStorage> {
    use_context::<Signal<PlaybackState, SyncStorage>>()
}

/// タイマースレッドの cancellation token を新規生成する。
#[must_use]
pub fn new_cancel_token() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// 次フレーム決定の結果。`spawn_playback_thread` 内部用。
enum NextStep {
    Set(usize),
    Stop,
}

/// `Frame.ticks` ぶん sleep しながら次のフレーム index を `pending_frame` に書き込むタイマースレッド。
/// tick → 実時間の換算は 60Hz vsync 想定 (`VSYNC_TICK = 1/60 秒`) で engine と統一。
///
/// 末尾到達時:
/// - `is_loop` = true ならば `loop_start_index` へジャンプして継続
/// - `is_loop` = false ならば `state` を `Stopped` に書き戻して終了
///
/// cancel フラグは 20ms 単位の chunk sleep ごとにチェックするので、Pause/Stop の体感反応は十分速い。
pub fn spawn_playback_thread(
    mut state: Signal<PlaybackState, SyncStorage>,
    mut pending_frame: Signal<Option<usize>, SyncStorage>,
    config: Signal<PlaybackConfig, SyncStorage>,
    initial_frame: usize,
    cancel: Arc<AtomicBool>,
) {
    // cancel チェック粒度。短すぎると wakeup 過多、長すぎると Pause/Stop の体感遅延になる。
    const CHUNK: Duration = Duration::from_millis(20);
    // ticks=0 のフレームに当たった時のフォールバック (busy loop 回避、最低 1 tick = 約 16.67ms)。
    const MIN_TICKS: u32 = 1;
    // engine の `animation::VSYNC_TICK` と同じ値。pkg をまたいで参照すると依存が大きく
    // なるので、editor 側にもローカルに同じ定数を置く。
    const VSYNC_TICK: Duration = Duration::from_micros(16_667);

    std::thread::spawn(move || {
        // スレッド内のローカル frame index。pending_frame に Some(idx) を書くと UI で
        // selected_frame_index に転送される (use_effect でブリッジ)。
        let mut current = initial_frame;

        loop {
            if cancel.load(Ordering::Relaxed) {
                return;
            }

            // 現在のフレーム ticks / loop 情報を Sync Signal から読む
            let (frame_ticks, total_frames, is_loop, loop_start) = {
                let cfg = config.peek();
                let t = cfg
                    .frame_ticks
                    .get(current)
                    .copied()
                    .unwrap_or(3)
                    .max(MIN_TICKS);
                (t, cfg.frame_ticks.len(), cfg.is_loop, cfg.loop_start_index)
            };

            // chunk 単位で sleep して cancel に反応する
            let total = VSYNC_TICK * frame_ticks;
            let mut elapsed = Duration::ZERO;
            while elapsed < total {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                // ループ条件で elapsed < total を保証しているが clippy 対策で saturating_sub を使う
                let remaining = total.saturating_sub(elapsed);
                let chunk = if remaining < CHUNK { remaining } else { CHUNK };
                std::thread::sleep(chunk);
                elapsed += chunk;
            }

            if cancel.load(Ordering::Relaxed) {
                return;
            }

            // 次フレームを計算
            let next = {
                let proposed = current.saturating_add(1);
                if proposed < total_frames {
                    NextStep::Set(proposed)
                } else if is_loop {
                    let start = usize::try_from(loop_start).unwrap_or(0);
                    if start < total_frames {
                        NextStep::Set(start)
                    } else {
                        NextStep::Stop
                    }
                } else {
                    NextStep::Stop
                }
            };

            match next {
                NextStep::Set(i) => {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    current = i;
                    pending_frame.set(Some(i));
                }
                NextStep::Stop => {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    // Loop しない (or loop_start_index 不正) で末尾に到達したときは
                    // 手動 Stop と同じく先頭フレームへ戻す。
                    pending_frame.set(Some(0));
                    state.set(PlaybackState::Stopped);
                    return;
                }
            }
        }
    });
}
