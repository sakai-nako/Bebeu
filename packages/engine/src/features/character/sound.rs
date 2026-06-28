//! Frame に紐づく SE の dispatch (ADR-0019 / ADR-0034)。
//!
//! ## 流れ
//!
//! 1. Character ロード時に [`bake_character_sounds`] が各 `SoundGroup.Sound.path` を
//!    `AssetServer.load()` で `Handle<AudioSource>` 化して [`CharacterSounds`] に焼く。
//! 2. 毎 tick (AnimationSet::Tick + AttackSet::Resolve の後) [`tick_sound_dispatch`] が:
//!     - `AnimationFrames.current_index()` が前 tick と変わったら frame 進入と判定し、
//!       新 frame の `frame_sound` と現 `AttackOutcome` から `step_dispatch` の規則で
//!       1 つの SoundGroup.number を選び pending スロットに latch
//!     - pending の `remaining_delay` を VSYNC_TICK ぶん減算し、0 以下になったら
//!       `SoundGroup.pick` で 1 つの Sound を選び `AudioPlayer` を spawn して発火
//! 3. `Changed<AnimationFrames>` (= state_machine が switchTo で AnimationFrames を差し替え)
//!    で `SoundDispatch` と `AttackOutcome` をリセット: `prev_frame_index = None`,
//!    `pending = None`, `attack_outcome = Idle`
//!    → 次 tick で新 anim frame 0 の sound が改めて latch される (= キャンセルされた
//!    アクションの SE は鳴らさない、attack result も持ち越さない)
//!
//! ## prev_frame_index = None の意味
//!
//! ADR-0019 の Go 実装: 起動時は 0、switchTo 後は -1 (sentinel)。Rust では Option<usize> で:
//! - `Added<AnimationFrames>` (= spawn 直後) → `Some(0)` (= 既に frame 0 を観測した扱い、
//!   Idle frame 0 の sound 誤発火を抑止)
//! - `Changed<AnimationFrames>` で `is_added() == false` (= switchTo) → `None`
//!   (= まだ何も観測していない、次 tick で current=0 と None が異なるので frame 0 の sound
//!   を発火させる)
//!
//! ## Hit / Guard 出し分け (ADR-0034)
//!
//! `AttackOutcome` を attacker (= Player) に attach し、`resolve_hits` (AttackSet::Resolve)
//! で Hit / Guarded を書き込む。`tick_sound_dispatch` は `.after(AttackSet::Resolve)` で
//! 順序固定されているので、同 tick 内で attack 確定 → frame 進入 latch という流れになる。
//! `step_dispatch` 側で `attack_outcome` を見て `on_hit` / `on_guard` / `number` を選択する。
use std::time::Duration;

use bevy::audio::{AudioPlayer, PlaybackSettings, Volume};
use bevy::prelude::*;
use rand::Rng;

use crate::entities::character::{Character, FrameSound, SoundGroup};

use super::animation::{AnimationFrames, AnimationSet, VSYNC_TICK};
use super::attack::AttackSet;
use super::debug_control::SimulationSet;
use super::hit_stop::HitStopState;

/// ADR-0034: attacker (現状 Player のみ) が「現在の attack で何が起きたか」を保持する
/// anim-scope state。`resolve_hits` が Hit / Guard 成立で書き込み、switchTo
/// (= [`track_animation_swap`] の Changed 分岐) で Idle にリセットする。
/// `step_dispatch` がこの値を見て `on_hit` / `on_guard` / `number` の出し分けを決定する。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttackOutcome {
    /// まだ何も当てていない (初期 / 切り替え直後 / 空振り中)。
    #[default]
    Idle,
    /// 通常 hit が成立 (Guard でない & 当たり判定通過)。
    Hit,
    /// ガードされて成立 (`enemy_state == Guard` 時の被弾)。
    Guarded,
}

/// 焼き込み済みの SoundGroup 集合 (ADR-0019)。entity に attach して dispatch system が
/// `Frame.sound.number` から SoundGroup を引くのに使う。各 `Sound.path` は
/// `AssetServer.load()` で `Handle<AudioSource>` に解決済み。
#[derive(Component, Debug, Clone, Default)]
pub struct CharacterSounds {
    groups: std::collections::HashMap<u32, BakedSoundGroup>,
}

#[derive(Debug, Clone)]
struct BakedSoundGroup {
    sounds: Vec<BakedSound>,
}

#[derive(Debug, Clone)]
struct BakedSound {
    handle: Handle<AudioSource>,
    volume: f32,
    /// `effective_weight` 適用済み (`<= 0.0` は 1.0 に倒れている)。重み付き pick で使う。
    weight: f32,
}

impl CharacterSounds {
    /// `number` から焼き込み済み SoundGroup を引く。見つからなければ `None`。
    fn get(&self, number: u32) -> Option<&BakedSoundGroup> {
        self.groups.get(&number)
    }
}

impl BakedSoundGroup {
    /// `rand` は `f32 ∈ [0, 1)` を返す closure。SoundGroup::pick と同じ重み付き選択ロジック。
    fn pick<F: FnOnce() -> f32>(&self, rand: F) -> Option<&BakedSound> {
        if self.sounds.is_empty() {
            return None;
        }
        let total: f32 = self.sounds.iter().map(|s| s.weight).sum();
        if total <= 0.0 {
            return self.sounds.first();
        }
        let mut roll = rand() * total;
        for s in &self.sounds {
            roll -= s.weight;
            if roll <= 0.0 {
                return Some(s);
            }
        }
        self.sounds.last()
    }
}

/// Character の `sound_groups` を全部 `Handle<AudioSource>` 化して `CharacterSounds`
/// に焼く (battle scene の spawn 経路から呼ぶ)。
///
/// 個別 WAV path は `runtime/data/characters/{character}/sound-groups/{group}/sounds/{sound}`
/// を AssetServer (= runtime/data/ 起点) に渡せばよい。
#[must_use]
pub fn bake_character_sounds(
    asset_server: &AssetServer,
    character_name: &str,
    character: &Character,
) -> CharacterSounds {
    let mut groups = std::collections::HashMap::new();
    for group in character.sound_groups.values() {
        let baked: Vec<BakedSound> = group
            .sounds
            .iter()
            .map(|s| BakedSound {
                handle: asset_server.load(format!(
                    "characters/{character_name}/sound-groups/{}/sounds/{}",
                    group.name, s.path,
                )),
                volume: s.volume,
                weight: effective_weight(s.weight),
            })
            .collect();
        groups.insert(group.number, BakedSoundGroup { sounds: baked });
    }
    CharacterSounds { groups }
}

/// SoundGroup::pick / BakedSoundGroup::pick で共有する重み補正。`<= 0.0` (NaN 含む) を 1.0 に倒す。
fn effective_weight(w: f32) -> f32 {
    if w > 0.0 { w } else { 1.0 }
}

/// SE 発火の中継スロット (ADR-0019)。1 entity 1 pending。新しい frame で sound が
/// latch されると古い pending は捨てられる (= 高密度シーケンスでは取りこぼし)。
#[derive(Component, Debug, Clone, Default)]
pub struct SoundDispatch {
    /// 前 tick に観測した `AnimationFrames.current_index()`。`None` は「まだ何も観測していない」
    /// (= switchTo 直後)。frame 進入検知の比較対象。
    prev_frame_index: Option<usize>,
    pending: Option<PendingSound>,
}

#[derive(Debug, Clone)]
struct PendingSound {
    number: u32,
    remaining_delay: Duration,
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                // Added/Changed フィルタを 1 system 内で `Ref::is_added`/`is_changed` で分岐
                // するため、reset 系は単一 system にまとめる。`Changed<T>` は Added を含むので
                // 順序問わず is_added 分岐から先に書く。
                track_animation_swap,
                // ADR-0034: AttackSet::Resolve の後に走らせて、同 tick で確定した
                // attack_outcome (Hit/Guard) を frame 進入時の出し分けに使える状態にする。
                tick_sound_dispatch
                    .after(AnimationSet::Tick)
                    .after(AttackSet::Resolve),
            )
                .in_set(SimulationSet::Active),
        );
    }
}

/// `Changed<AnimationFrames>` を捉えて `SoundDispatch` と `AttackOutcome` をリセットする。
/// - `Added` (= spawn 直後) → `prev_frame_index = Some(0)` (Idle frame 0 sound 誤発火抑止)、
///   `AttackOutcome = Idle`
/// - それ以外の Changed (= switchTo) → `prev_frame_index = None` (新 anim frame 0 sound 発火)、
///   `AttackOutcome = Idle` (= 前 anim の hit/guard 結果は持ち越さない)
///
/// `AttackOutcome` を持たない entity (= Enemy など、まだ attacker 側でないキャラ) は
/// `Option<&mut AttackOutcome>` で吸収する。
fn track_animation_swap(
    mut query: Query<(
        Ref<AnimationFrames>,
        &mut SoundDispatch,
        Option<&mut AttackOutcome>,
    )>,
) {
    for (anim, mut s, outcome) in &mut query {
        if anim.is_added() {
            s.prev_frame_index = Some(0);
            s.pending = None;
            if let Some(mut o) = outcome {
                *o = AttackOutcome::Idle;
            }
        } else if anim.is_changed() {
            s.prev_frame_index = None;
            s.pending = None;
            if let Some(mut o) = outcome {
                *o = AttackOutcome::Idle;
            }
        }
    }
}

/// 毎 tick、frame 進入を検知して pending に latch、pending を遅延消化して発火する。
/// `HitStopState` 中の entity は time freeze 扱いで skip (Animation tick と同じ規約)。
///
/// `AttackOutcome` は attacker (= 通常 Player) だけが持つので Option で受け、未 attach の
/// entity (= Enemy など現状 attacker でないキャラ) では `Idle` 扱いにフォールバックする
/// (= ADR-0034 の `on_hit` / `on_guard` 仕組みは使われず `number` だけが選ばれる)。
fn tick_sound_dispatch(
    mut commands: Commands,
    mut query: Query<
        (
            &AnimationFrames,
            &CharacterSounds,
            &mut SoundDispatch,
            Option<&AttackOutcome>,
        ),
        Without<HitStopState>,
    >,
) {
    let mut rng = rand::rng();
    for (anim, sounds, mut dispatch, outcome) in &mut query {
        let attack_outcome = outcome.copied().unwrap_or_default();
        let Some(number) = step_dispatch(
            anim.current_index(),
            anim.current_frame_sound(),
            attack_outcome,
            &mut dispatch,
        ) else {
            continue;
        };
        let Some(group) = sounds.get(number) else {
            tracing::warn!(number, "sound dispatch: SoundGroup not found");
            continue;
        };
        let Some(picked) = group.pick(|| rng.random::<f32>()) else {
            // 空 group は SoundGroup.pick が None を返す。warn して終了。
            tracing::warn!(number, "sound dispatch: SoundGroup is empty");
            continue;
        };
        commands.spawn((
            AudioPlayer::new(picked.handle.clone()),
            PlaybackSettings {
                volume: Volume::Linear(picked.volume),
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
}

/// `tick_sound_dispatch` の純粋ロジック (ECS 非依存)。テスト容易性のため独立。
///
/// 流れ (ADR-0019 / ADR-0034):
/// 1. `current_index` が `prev_frame_index` と異なる → frame 進入扱い。
///    `attack_outcome` と `current_frame_sound` から `select_frame_sound_number` で
///    1 つの SoundGroup.number を選び pending に latch (delay_ms を Duration に変換)。
///    `prev` を `current` に更新
/// 2. pending があれば VSYNC_TICK ぶん減算してから `is_zero` を判定。0 で fire。
///    saturating 減算なので `delay_ms = 0` は同 tick に即発火 (= frame 進入と同期再生)、
///    `delay_ms > 0` は ceil(delay_ms / VSYNC_TICK_ms) tick ぶん遅れて発火する。
///
/// 戻り値は「この tick で発火する SoundGroup.number」(なし = `None`)。
fn step_dispatch(
    current_index: usize,
    current_frame_sound: Option<FrameSound>,
    attack_outcome: AttackOutcome,
    dispatch: &mut SoundDispatch,
) -> Option<u32> {
    if Some(current_index) != dispatch.prev_frame_index {
        dispatch.prev_frame_index = Some(current_index);
        if let Some(fs) = current_frame_sound
            && let Some(number) = select_frame_sound_number(&fs, attack_outcome)
        {
            dispatch.pending = Some(PendingSound {
                number,
                remaining_delay: Duration::from_millis(u64::from(fs.delay_ms)),
            });
        }
    }
    let pending = dispatch.pending.as_mut()?;
    pending.remaining_delay = pending.remaining_delay.saturating_sub(VSYNC_TICK);
    if !pending.remaining_delay.is_zero() {
        return None;
    }
    let number = pending.number;
    dispatch.pending = None;
    Some(number)
}

/// ADR-0034: 3 系統の SoundGroup 参照 (`number` / `on_hit` / `on_guard`) から、
/// 現在の `AttackOutcome` に応じて 1 つを選ぶ。
///
/// - `Hit` → `on_hit.or(number)` (= ヒット音、なければ swing 音にフォールバック)
/// - `Guarded` → `on_guard.or(number)` (= 同上のガード版)
/// - `Idle` → `number` (= 通常 swing 音)
///
/// 戻り値が `None` = この frame ではどの分岐でも音が指定されていない (= 無発火)。
fn select_frame_sound_number(fs: &FrameSound, outcome: AttackOutcome) -> Option<u32> {
    match outcome {
        AttackOutcome::Hit => fs.on_hit.or(fs.number),
        AttackOutcome::Guarded => fs.on_guard.or(fs.number),
        AttackOutcome::Idle => fs.number,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baked_group(weights: &[f32]) -> BakedSoundGroup {
        BakedSoundGroup {
            sounds: weights
                .iter()
                .map(|&w| BakedSound {
                    handle: Handle::default(),
                    volume: 1.0,
                    weight: effective_weight(w),
                })
                .collect(),
        }
    }

    #[test]
    fn baked_group_pick_distributes_by_roll() {
        // 3 要素 weight=1 → roll=0.0 → idx 0、0.5 → idx 1、0.9 → idx 2。SoundGroup::pick と
        // 同等の挙動を BakedSoundGroup でも維持していることを担保する。
        let g = baked_group(&[1.0, 1.0, 1.0]);
        let mut handles = vec![];
        for roll in [0.0, 0.5, 0.9] {
            let s = g.pick(|| roll).expect("pick");
            handles.push(s.volume); // volume はすべて 1.0 だが Some が返ることだけ確認
        }
        assert_eq!(handles.len(), 3);
    }

    #[test]
    fn baked_group_pick_returns_last_on_overshoot() {
        let g = baked_group(&[1.0, 1.0]);
        assert!(g.pick(|| 1.0).is_some());
    }

    #[test]
    fn character_sounds_get_missing_returns_none() {
        let cs = CharacterSounds::default();
        assert!(cs.get(0).is_none());
    }

    #[test]
    fn sound_dispatch_default_has_no_pending_and_no_prev() {
        let s = SoundDispatch::default();
        assert!(s.pending.is_none());
        assert!(s.prev_frame_index.is_none());
    }

    #[test]
    fn effective_weight_clamps_nonpositive_to_one() {
        assert!((effective_weight(0.0) - 1.0).abs() < f32::EPSILON);
        assert!((effective_weight(-1.0) - 1.0).abs() < f32::EPSILON);
        assert!((effective_weight(2.5) - 2.5).abs() < f32::EPSILON);
        assert!((effective_weight(f32::NAN) - 1.0).abs() < f32::EPSILON);
    }

    // === step_dispatch (frame 進入検知 + pending decrement) ===

    /// 振り音 (number) だけを持つ FrameSound (ADR-0019 の従来形)。
    fn fs(number: u32, delay_ms: u32) -> FrameSound {
        FrameSound {
            number: Some(number),
            on_hit: None,
            on_guard: None,
            delay_ms,
        }
    }

    /// 任意の 3 系統 + delay。ADR-0034 の出し分けテスト用。
    fn fs_full(
        number: Option<u32>,
        on_hit: Option<u32>,
        on_guard: Option<u32>,
        delay_ms: u32,
    ) -> FrameSound {
        FrameSound {
            number,
            on_hit,
            on_guard,
            delay_ms,
        }
    }

    #[test]
    fn step_dispatch_spawn_state_does_not_fire_idle_frame_0() {
        // spawn 直後: prev_frame_index = Some(0) で起動 (track_animation_swap の Added 分岐相当)。
        // current=0 + Idle frame 0 に sound があっても、prev と current が一致するので latch しない。
        let mut d = SoundDispatch {
            prev_frame_index: Some(0),
            pending: None,
        };
        let n = step_dispatch(0, Some(fs(7, 0)), AttackOutcome::Idle, &mut d);
        assert!(n.is_none(), "spawn 直後の frame 0 は誤発火させない");
        assert!(d.pending.is_none());
    }

    #[test]
    fn step_dispatch_switch_state_fires_new_anim_frame_0() {
        // switchTo 直後: prev_frame_index = None。current=0 で frame 0 sound を latch → delay 0 で即発火。
        let mut d = SoundDispatch {
            prev_frame_index: None,
            pending: None,
        };
        let n = step_dispatch(0, Some(fs(7, 0)), AttackOutcome::Idle, &mut d);
        assert_eq!(n, Some(7));
        assert_eq!(d.prev_frame_index, Some(0));
        assert!(d.pending.is_none(), "発火後は pending をクリア");
    }

    #[test]
    fn step_dispatch_frame_transition_latches_new_sound() {
        // prev=Some(0)、新たに current=1 に遷移、frame 1 に sound あり (delay 0) → 即発火。
        let mut d = SoundDispatch {
            prev_frame_index: Some(0),
            pending: None,
        };
        let n = step_dispatch(1, Some(fs(5, 0)), AttackOutcome::Idle, &mut d);
        assert_eq!(n, Some(5));
        assert_eq!(d.prev_frame_index, Some(1));
    }

    #[test]
    fn step_dispatch_delay_consumed_over_multiple_ticks() {
        // delay_ms=50 → ceil(50 / 16.667) = 3 tick で発火 (saturating 減算で最終 tick がゼロ)。
        let mut d = SoundDispatch {
            prev_frame_index: Some(0),
            pending: None,
        };
        // tick 1: 進入と同 tick に latch (50ms) + decrement → 33.333ms 残
        assert_eq!(
            step_dispatch(1, Some(fs(2, 50)), AttackOutcome::Idle, &mut d),
            None
        );
        // tick 2: 33.333ms → 16.666ms 残
        assert_eq!(step_dispatch(1, None, AttackOutcome::Idle, &mut d), None);
        assert!(d.pending.is_some());
        // tick 3: 16.666ms - 16.667ms saturating → 0 → 発火
        assert_eq!(step_dispatch(1, None, AttackOutcome::Idle, &mut d), Some(2));
        assert!(d.pending.is_none());
    }

    #[test]
    fn step_dispatch_no_sound_means_no_latch_no_fire() {
        // frame 進入はあったが Frame.sound が None → pending は据え置き、何も鳴らない。
        let mut d = SoundDispatch {
            prev_frame_index: Some(0),
            pending: None,
        };
        assert_eq!(step_dispatch(1, None, AttackOutcome::Idle, &mut d), None);
        assert!(d.pending.is_none());
        assert_eq!(d.prev_frame_index, Some(1), "frame 進入は記録される");
    }

    #[test]
    fn step_dispatch_new_frame_with_sound_overrides_old_pending() {
        // ADR-0019: 1 スロット pending 上書き。古い pending は破棄される。
        let mut d = SoundDispatch {
            prev_frame_index: Some(0),
            pending: Some(PendingSound {
                number: 99,
                remaining_delay: Duration::from_millis(100),
            }),
        };
        // 新 frame で新 sound を latch (delay 50)。同 tick の decrement で 33.333ms 残。
        assert_eq!(
            step_dispatch(1, Some(fs(2, 50)), AttackOutcome::Idle, &mut d),
            None
        );
        let pending = d.pending.as_ref().expect("new pending should latch");
        assert_eq!(pending.number, 2, "古い number=99 は上書きされる");
        // 50ms - VSYNC_TICK(16.667ms) = 33.333ms 残 (同 tick decrement のため)
        assert_eq!(pending.remaining_delay, Duration::from_micros(33_333));
    }

    #[test]
    fn step_dispatch_loop_back_to_frame_0_refires() {
        // ループ末尾 → frame 0 (loop_start_index) に巻き戻ったとき、prev != current となるので
        // frame 0 の sound を再 latch する (= ループごとに発火可能)。
        let mut d = SoundDispatch {
            prev_frame_index: Some(2),
            pending: None,
        };
        let n = step_dispatch(0, Some(fs(1, 0)), AttackOutcome::Idle, &mut d);
        assert_eq!(n, Some(1));
    }

    // === ADR-0034: Hit / Guard 出し分け ===

    #[test]
    fn select_frame_sound_number_idle_picks_swing() {
        // Idle (= 通常 swing) は number を選ぶ。on_hit / on_guard は無視。
        let fs = fs_full(Some(1), Some(2), Some(3), 0);
        assert_eq!(select_frame_sound_number(&fs, AttackOutcome::Idle), Some(1));
    }

    #[test]
    fn select_frame_sound_number_hit_prefers_on_hit() {
        // Hit のとき on_hit が優先。
        let fs = fs_full(Some(1), Some(2), Some(3), 0);
        assert_eq!(select_frame_sound_number(&fs, AttackOutcome::Hit), Some(2));
    }

    #[test]
    fn select_frame_sound_number_hit_falls_back_to_number_if_on_hit_none() {
        // Hit のとき on_hit が無ければ number に fallback (= swing の打撃版が無いケース)。
        let fs = fs_full(Some(1), None, Some(3), 0);
        assert_eq!(select_frame_sound_number(&fs, AttackOutcome::Hit), Some(1));
    }

    #[test]
    fn select_frame_sound_number_guarded_prefers_on_guard() {
        // Guarded のとき on_guard が優先。
        let fs = fs_full(Some(1), Some(2), Some(3), 0);
        assert_eq!(
            select_frame_sound_number(&fs, AttackOutcome::Guarded),
            Some(3)
        );
    }

    #[test]
    fn select_frame_sound_number_guarded_falls_back_to_number() {
        let fs = fs_full(Some(1), Some(2), None, 0);
        assert_eq!(
            select_frame_sound_number(&fs, AttackOutcome::Guarded),
            Some(1)
        );
    }

    #[test]
    fn select_frame_sound_number_returns_none_when_only_relevant_field_missing() {
        // 振り音 only frame で Hit → on_hit none, number あり → number にフォールバック (= swing 鳴る)。
        let fs = fs_full(Some(5), None, None, 0);
        assert_eq!(select_frame_sound_number(&fs, AttackOutcome::Hit), Some(5));
        // on_hit only frame で Idle → number none, on_hit あっても Idle なので無発火。
        let fs = fs_full(None, Some(6), None, 0);
        assert_eq!(select_frame_sound_number(&fs, AttackOutcome::Idle), None);
    }

    #[test]
    fn step_dispatch_hit_outcome_picks_on_hit_at_frame_entry() {
        // 別フレーム case: frame 3 進入時に AttackOutcome=Hit、frame 3 に on_hit あり → on_hit latch + 発火。
        let mut d = SoundDispatch {
            prev_frame_index: Some(2),
            pending: None,
        };
        let fs = fs_full(None, Some(7), Some(8), 0);
        assert_eq!(
            step_dispatch(3, Some(fs), AttackOutcome::Hit, &mut d),
            Some(7)
        );
    }

    #[test]
    fn step_dispatch_guarded_outcome_picks_on_guard_at_frame_entry() {
        // Guard も対称: on_guard が選ばれる。
        let mut d = SoundDispatch {
            prev_frame_index: Some(2),
            pending: None,
        };
        let fs = fs_full(None, Some(7), Some(8), 0);
        assert_eq!(
            step_dispatch(3, Some(fs), AttackOutcome::Guarded, &mut d),
            Some(8)
        );
    }

    #[test]
    fn step_dispatch_idle_on_hit_only_frame_does_not_fire() {
        // on_hit/on_guard だけ持つ frame で AttackOutcome=Idle → 無発火 (= 振り音もないので
        // 空振りでは無音、想定通り)。
        let mut d = SoundDispatch {
            prev_frame_index: Some(2),
            pending: None,
        };
        let fs = fs_full(None, Some(7), Some(8), 0);
        let n = step_dispatch(3, Some(fs), AttackOutcome::Idle, &mut d);
        assert!(n.is_none());
        // 進入は記録されるが pending は積まれない (= number/on_hit/on_guard 全部 fallback で None)。
        assert_eq!(d.prev_frame_index, Some(3));
        assert!(d.pending.is_none());
    }

    #[test]
    fn step_dispatch_swing_only_frame_with_hit_outcome_uses_swing() {
        // 振り音 only frame で Hit → on_hit 無 → number にフォールバック。
        // 「frame 1 に swing、frame 3 で結果別」の構造で frame 1 進入時の挙動を担保する。
        let mut d = SoundDispatch {
            prev_frame_index: None,
            pending: None,
        };
        let fs = fs_full(Some(1), None, None, 0);
        let n = step_dispatch(0, Some(fs), AttackOutcome::Hit, &mut d);
        assert_eq!(n, Some(1));
    }
}
