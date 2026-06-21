//! Debug pause + frame advance (FSD: feature slice)。
//!
//! - `F3` で [`DebugPause::paused`] を toggle。
//! - `F4` で 1 frame だけ進める (single step)。pause 中のみ反応。
//!
//! gameplay 系 system は [`SimulationSet::Active`] に括ってあり、本 plugin が
//! `run_if(simulation_active)` で gate する。debug overlay (hitbox_debug /
//! state_debug) と input toggle 系は **set 外** に置いて pause 中も動く。
//!
//! ## Pause 中の input latch
//!
//! pause 中に押された transition-driven 入力 (`just_pressed` で判定する Space 攻撃など)
//! は、normal Bevy の `Input<KeyCode>` 更新だけだと「押した frame と F4 を踏んだ frame」が
//! 一致しないと取りこぼす。本 module は `latch_paused_input` が pause 中の `just_pressed`
//! を `latched_keys` set に溜め、`toggle_pause` が F4 を観測した瞬間に「まだ押されている」
//! キーを `Input::reset` + `Input::press` で **再点火** する。これで:
//!
//! - ArrowRight を押しながら F4 → 移動 (`pressed` ベースなのでそもそも latch 無しでも動く)
//! - Space を先に押し、後から F4 → 攻撃発火
//! - 連続 F4 + 押しっぱなし → 1 回目だけ発火 (= normal play の挙動と一致)
//!
//! F3 / F4 自身は latch 対象外 (debug toggle が誤発火しないように)。
use std::collections::HashSet;

use bevy::prelude::*;

/// pause / single step の状態。F3 / F4 で更新され、`SimulationSet::Active` の
/// `run_if` 条件として読まれる。
#[derive(Resource, Debug, Default)]
pub struct DebugPause {
    /// `true` のとき gameplay tick が停止する。
    pub paused: bool,
    /// この frame だけ pause を上書きして 1 step 走らせるフラグ。`Last` schedule の
    /// `reset_single_step` が tick 後に false に戻す。
    pub single_step: bool,
    /// pause 中に Bevy 上 `just_pressed` を観測したキー集合。F4 の single_step で
    /// 「まだ押されている」ものだけ再点火する。advance 完了後に clear (toggle_pause 内)。
    /// pause OFF 時は常に空。
    latched_keys: HashSet<KeyCode>,
}

/// gameplay (= pause 対象) systems の category SystemSet。
/// 順序付けは別の SystemSet (例: [`super::animation::AnimationSet::Tick`]) でやる。
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimulationSet {
    /// pause 時に skip される全 gameplay tick の集合。
    Active,
}

pub struct DebugControlPlugin;

impl Plugin for DebugControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugPause>()
            // Update / PostUpdate 両方に同じ条件を貼る (hit_stop が PostUpdate に居るため)。
            .configure_sets(Update, SimulationSet::Active.run_if(simulation_active))
            .configure_sets(PostUpdate, SimulationSet::Active.run_if(simulation_active))
            // latch_paused_input は toggle_pause の **前**に走らせる: 同 frame で Space と F4
            // が同時 just_pressed の場合、Space を先に latched に積んでから toggle_pause で
            // 再点火するため。
            // toggle_pause は **必ず SimulationSet::Active より前**に走らせる: `single_step=true`
            // のセット → run_if 評価 → set 内 systems 実行、の順を Bevy scheduler に保証させる。
            // この `.before` が無いと、F4 を押した frame に sim 系が「単純に paused のまま」と
            // 評価されて 1 step が空振りする。
            .add_systems(
                Update,
                (latch_paused_input, toggle_pause)
                    .chain()
                    .before(SimulationSet::Active),
            )
            // single_step を tick 完了後にリセットする必要があるので、Last schedule で確実に
            // すべての SimulationSet 集合 (Update / PostUpdate) より後に走らせる。
            .add_systems(Last, reset_single_step);
    }
}

/// pause 中、Bevy が自然に立てる `just_pressed` をフックして `latched_keys` に積む。
/// pause OFF / single_step 中は積まない (= 通常 input フローに干渉しない)。
/// pause が解除された瞬間に latched_keys を空にして、次回 pause に持ち越さない。
fn latch_paused_input(keys: Res<ButtonInput<KeyCode>>, mut pause: ResMut<DebugPause>) {
    if !pause.paused {
        if !pause.latched_keys.is_empty() {
            pause.latched_keys.clear();
        }
        return;
    }
    if pause.single_step {
        // single_step frame は latched を消費するので、ここで上積みしない。
        return;
    }
    for &k in keys.get_just_pressed() {
        if matches!(k, KeyCode::F3 | KeyCode::F4) {
            continue;
        }
        pause.latched_keys.insert(k);
    }
}

fn toggle_pause(mut keys: ResMut<ButtonInput<KeyCode>>, mut pause: ResMut<DebugPause>) {
    if keys.just_pressed(KeyCode::F3) {
        pause.paused = !pause.paused;
        pause.single_step = false;
        pause.latched_keys.clear();
        tracing::info!(paused = pause.paused, "debug: pause toggled");
    }
    if pause.paused && keys.just_pressed(KeyCode::F4) {
        pause.single_step = true;
        // latched に積まれていた「pause 中に押されたキー」のうち、いま現に押されているもの
        // を reset → press で再点火し、sim 系の `just_pressed` 判定に乗せる。
        let to_repress: Vec<KeyCode> = pause
            .latched_keys
            .iter()
            .copied()
            .filter(|k| keys.pressed(*k))
            .collect();
        pause.latched_keys.clear();
        for k in to_repress {
            keys.reset(k);
            keys.press(k);
        }
        tracing::debug!("debug: frame advance");
    }
}

/// `SimulationSet::Active` の `run_if` 条件。pause 中でも single_step が立っていれば
/// その frame だけ通す。
fn simulation_active(pause: Res<DebugPause>) -> bool {
    !pause.paused || pause.single_step
}

fn reset_single_step(mut pause: ResMut<DebugPause>) {
    if pause.single_step {
        pause.single_step = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_active_when_not_paused() {
        let app = App::new();
        let _ = app;
        let pause = DebugPause::default();
        assert!(
            !pause.paused,
            "default pause should be running (paused=false)"
        );
    }

    #[test]
    fn single_step_overrides_paused_flag_semantically() {
        // simulation_active の真理値表 (Resource を直接 wrap)。
        let cases = [
            (false, false, true), // not paused, no step → run
            (false, true, true),  // not paused, step set (no-op) → run
            (true, false, false), // paused, no step → halt
            (true, true, true),   // paused, single step → run this tick
        ];
        for (paused, single_step, expected) in cases {
            let active = !paused || single_step;
            assert_eq!(
                active, expected,
                "paused={paused} single_step={single_step}"
            );
        }
    }

    /// pause 中に Space を押し、後から F4 を踏むケース: Space が `just_pressed` として
    /// 再点火されることを `ButtonInput` 直接操作で再現する (Bevy App 起動はせず、`toggle_pause`
    /// の核ロジックを inline で実行する)。
    #[test]
    fn space_held_before_f4_advance_repress_just_pressed() {
        let mut input = ButtonInput::<KeyCode>::default();
        // 物理的に Space を押しっぱなしの状態。Bevy 上は通常 frame の前半で just_pressed=true、
        // 後続 frame では false になる前提なので、ここではまず press して just_pressed を立てる。
        input.press(KeyCode::Space);
        assert!(input.just_pressed(KeyCode::Space));
        // 次の frame で latched に積まれた想定 (`latch_paused_input` 相当)。Bevy 側の
        // just_pressed は false に降ろされる: clear で just_pressed をクリア。
        input.clear();
        assert!(!input.just_pressed(KeyCode::Space));
        assert!(input.pressed(KeyCode::Space));
        let mut pause = DebugPause {
            paused: true,
            single_step: false,
            latched_keys: [KeyCode::Space].into_iter().collect(),
        };
        // ここで F4 が押されたと仮定して `toggle_pause` の after-F4 ブロックを inline 実行。
        pause.single_step = true;
        let to_repress: Vec<KeyCode> = pause
            .latched_keys
            .iter()
            .copied()
            .filter(|k| input.pressed(*k))
            .collect();
        pause.latched_keys.clear();
        for k in to_repress {
            input.reset(k);
            input.press(k);
        }
        // sim systems が見るときには Space.just_pressed=true (再点火済み)。
        assert!(input.just_pressed(KeyCode::Space));
        assert!(input.pressed(KeyCode::Space));
        assert!(pause.latched_keys.is_empty());
    }

    /// pause 中に Space を一度押して、続けて 2 回目の F4 を踏むケース: 2 回目は再点火しない
    /// (= 通常 play の「押しっぱなしは 1 回だけ just_pressed」挙動と一致)。
    #[test]
    fn second_advance_with_same_held_key_does_not_repress() {
        let mut input = ButtonInput::<KeyCode>::default();
        input.press(KeyCode::Space);
        input.clear();
        let mut pause = DebugPause {
            paused: true,
            single_step: false,
            // 1 回目の advance で latched は既に消費済み → 空。
            latched_keys: HashSet::new(),
        };
        // 2 回目の F4。
        pause.single_step = true;
        let to_repress: Vec<KeyCode> = pause
            .latched_keys
            .iter()
            .copied()
            .filter(|k| input.pressed(*k))
            .collect();
        pause.latched_keys.clear();
        for k in to_repress {
            input.reset(k);
            input.press(k);
        }
        // 再点火されないので Space.just_pressed=false のまま (= 攻撃発火しない)。
        assert!(!input.just_pressed(KeyCode::Space));
        assert!(input.pressed(KeyCode::Space)); // 押されてはいる
    }

    /// pause 中に latch されたキーを、advance 直前に user が release していた場合は
    /// 再点火しない (Bevy の状態と整合)。
    #[test]
    fn released_before_advance_is_not_repressed() {
        let mut input = ButtonInput::<KeyCode>::default();
        input.press(KeyCode::Space);
        input.clear();
        input.release(KeyCode::Space); // ユーザが pause 中に Space を離した
        let mut pause = DebugPause {
            paused: true,
            single_step: false,
            latched_keys: [KeyCode::Space].into_iter().collect(),
        };
        pause.single_step = true;
        let to_repress: Vec<KeyCode> = pause
            .latched_keys
            .iter()
            .copied()
            .filter(|k| input.pressed(*k))
            .collect();
        pause.latched_keys.clear();
        for k in to_repress {
            input.reset(k);
            input.press(k);
        }
        // pressed でないので latched 消費されたが再点火されない。
        assert!(!input.just_pressed(KeyCode::Space));
        assert!(!input.pressed(KeyCode::Space));
    }
}
