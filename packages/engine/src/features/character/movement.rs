//! Movement feature。Camera を player に X 方向追従させ、`Facing` に応じて sprite を左右反転
//! する system 群、および手動入力 `PlayerInputController` を提供する。
//!
//! ADR-0035 で `handle_input` を 2 段に分割した:
//! - [`player_input_controller`] (本 module) — `ButtonInput` → 自 entity の [`AiCommand`]。
//!   ADR-0038 で `Controller::Human` 持ちの entity だけを扱う薄い system に変えた。
//! - [`super::ai::apply_command`] (ai module) — `AiCommand` → `CharacterState` /
//!   `KinematicVel` / `Facing` / `WorldPosition`。優先度判定と `is_locked()` skip はここに集約。
//!
//! ADR-0038 で旧 `Player(PlayerId)` / `Enemy` / `Ally` の 3 marker を [`Side`] / [`Controller`]
//! の 2 enum component に直交化した。`PlayerId` は引き続き `Controller::Human` 側の entity に
//! 直接 attach され、HUD の `target: p1` の引き先になる。
//!
//! ADR-0023 の world 軸:
//! - 左右: `world_x` (+ = 右)
//! - 奥行: `world_z` (+ = 手前 = 画像下)
//! - 高さ: `world_y` (jump 用、本 system では触らない)
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::entities::project::Project;
use crate::shared::PlayerId;
use crate::shared::projection;
use crate::shared::{Action, ActionMap};

use super::ai::{AiCommand, AiSet, BotBrain};
use super::animation::{AnimationFrames, AnimationSet};
use super::debug_control::SimulationSet;

/// ADR-0038: キャラクターの **陣営 (faction)**。攻撃対象 / HUD target / Brain target の
/// 起点になる。`Hero` 側は HUD で HP bar が出る側 (= 旧 `Player` + `Ally`)、`Villain` 側は
/// 殴られる側 (= 旧 `Enemy`)。同 side 内では damage / knockback は発生しない (attack resolve
/// で `attacker.side != victim.side` の組だけ計算)。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// HUD で HP bar を出す側。旧 `Player` / `Ally` marker を持っていた entity。
    Hero,
    /// 被弾対象として描画される側。旧 `Enemy` marker を持っていた entity。
    Villain,
}

/// ADR-0038: キャラクターの **操作主体 (controller)**。`Human` は手動入力
/// (`PlayerInputController`) を受け付ける entity、`Ai` は Brain (Melee/Ally/Bot) が
/// `AiCommand` を書き込む entity。Side と直交する 2 軸目で、(Hero, Human) = 旧 Player、
/// (Hero, Ai) = 旧 Ally、(Villain, Ai) = 旧 Enemy。
/// (Villain, Human) は将来の Mind Control 用に予約 (本 Issue では使われない)。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Controller {
    /// 手動入力。`PlayerInputController` system が ButtonInput → `AiCommand` を書き込む。
    Human,
    /// AI Brain。Side ごとに `MeleeBrain` (Villain) / `AllyBrain` (Hero) / `BotBrain`
    /// (Hero、Player 自動化) のいずれかが `AiCommand` を書き込む。
    Ai,
}

/// Character YAML の `tag` を持つ enemy にだけ attach される識別ラベル (ADR-0031)。
/// HUD の `enemy_hp_bar` が `target: { tag: "boss" }` で参照する。
/// tag が無いキャラには component 自体が無い (HUD 側は `Option<&EnemyTag>` で扱う)。
#[derive(Component, Debug, Clone)]
pub struct EnemyTag(pub String);

/// Player が直近で engagement した Enemy entity (ADR-0031)。HUD の engagement-link
/// 系 enemy bar が `target: { last_engaged_by: p1 }` で参照する。
/// 現状 Phase A は Player → Enemy 方向の hit でだけ書き込まれる (Enemy → Player の
/// 被弾はまだ未実装)。Player ごとに 1 component 持ち、初期値は `None`。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LastEngagedWith(pub Option<Entity>);

/// battle viewport を映している Camera2d を識別する marker component。
#[derive(Component, Debug, Clone, Copy)]
pub struct MainCamera;

/// ADR-0023 の world 座標を f32 で保持する component (連続移動用)。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct WorldPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl WorldPosition {
    #[must_use]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// player の向き。sprite の左右反転に焼き込む。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Facing {
    #[default]
    Right,
    Left,
}

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        // ADR-0035: 入力処理を `PlayerInputController` (ButtonInput → AiCommand) +
        // `super::ai::apply_command` (AiCommand → state) の 2 段に分割。本 plugin は
        // Player 入力読み取りと sync_* / camera_follow を担当する。
        //
        // 60Hz 固定 (ai::apply_command 側で dt = VSYNC_TICK_SECS を採用)。FixedUpdate は
        // wall-clock accumulator で 1 render に 0/2 回走るドリフトが出るので、Update に乗せて
        // dt を hardcode するのが「frame = vsync = 1 step」を最も素直に実現できる。
        // `player_input_controller` は AiSet::ReadInputs だけ in_set する (AiSet::ReadInputs
        // 自体が SimulationSet::Active の子なので、ここで Active を重ねると schedule warning
        // "redundant edge ... longer path exists" が出る)。
        app.add_systems(Update, player_input_controller.in_set(AiSet::ReadInputs));
        // sync_anchor / sync_flip は AnimationFrames の frame 切替後に走らないと
        // 「新 sprite.image + 旧 anchor/flip」の 1 frame ミスマッチが出る。
        // apply_command (AiSet::Apply) が state / position を書いたあとに sync_* が走る必要が
        // あるので `.after(AiSet::Apply)` も付ける。
        app.add_systems(
            Update,
            (sync_transform, sync_flip, sync_anchor, camera_follow)
                .after(AiSet::Apply)
                .after(AnimationSet::Tick)
                .in_set(SimulationSet::Active),
        );
    }
}

/// `ButtonInput` を読んで自 entity の [`AiCommand`] に書き込む薄い system (ADR-0035 の
/// Intent 層書き込み Human 側)。挙動は ADR-0035 Phase 1 と同じ:
/// - 移動: pressed (押下中継続) を ±1.0 デジタル値で `move_x` / `move_z` に乗せる
/// - 攻撃 / 下段攻撃 / ジャンプ: `just_pressed` を `attack` / `down_attack` / `jump` に乗せる
///   (= 押した瞬間 1 frame だけ true)
/// - ガード: `pressed` を `guard` に乗せる (押下中継続)
///
/// `face` は常に None で出し、Facing は `apply_command` 側で `move_x` の符号で更新される
/// (= 元 `handle_input` と同じ規約)。
///
/// ADR-0038: filter は `Controller::Human` 持ちの entity 全部 (= 旧 `Player` marker と
/// 等価)。Phase 3 の排他規約 `Without<BotBrain>` は維持 — Hero side / Human controller の
/// entity に BotBrain を attach すれば手動入力 system が自然に skip し、Brain が
/// `AiCommand` を上書きする。
fn player_input_controller(
    keys: Res<ButtonInput<KeyCode>>,
    action_map: Res<ActionMap>,
    mut query: Query<(&mut AiCommand, &Controller), Without<BotBrain>>,
) {
    let mut move_x = 0.0;
    if action_map.pressed(&keys, Action::MoveRight) {
        move_x += 1.0;
    }
    if action_map.pressed(&keys, Action::MoveLeft) {
        move_x -= 1.0;
    }
    let mut move_z = 0.0;
    if action_map.pressed(&keys, Action::MoveDown) {
        move_z += 1.0;
    }
    if action_map.pressed(&keys, Action::MoveUp) {
        move_z -= 1.0;
    }
    let attack = action_map.just_pressed(&keys, Action::Attack);
    let down_attack = action_map.just_pressed(&keys, Action::DownAttack);
    let jump = action_map.just_pressed(&keys, Action::Jump);
    let guard = action_map.pressed(&keys, Action::Guard);
    for (mut cmd, controller) in &mut query {
        if !matches!(controller, Controller::Human) {
            continue;
        }
        cmd.move_x = move_x;
        cmd.move_z = move_z;
        cmd.attack = attack;
        cmd.down_attack = down_attack;
        cmd.jump = jump;
        cmd.guard = guard;
        cmd.face = None;
    }
}

/// X 軸 → Z 軸の順に試して、壁にぶつかった軸だけ移動量を捨てる。
/// 対角入力で台形 area の斜辺に沿って滑る挙動を出す。
#[must_use]
pub fn step_axis_aware(
    pos: WorldPosition,
    dx: f32,
    dz: f32,
    contains: impl Fn(f32, f32) -> bool,
) -> WorldPosition {
    let mut next = pos;
    let candidate_x = next.x + dx;
    if contains(candidate_x, next.z) {
        next.x = candidate_x;
    }
    let candidate_z = next.z + dz;
    if contains(next.x, candidate_z) {
        next.z = candidate_z;
    }
    next
}

/// WorldPosition は f32 連続値だが、画像描画は Nearest filter (Bevy default) なので
/// transform を整数 pixel に snap してから projection に渡す。
/// snap 無しだと world x の sub-pixel 揺らぎが nearest snap の境界 (.5) をまたぐ
/// 瞬間にだけ sprite が 1 px 動いて見え、vsync の delta jitter と相互作用して
/// 「frame ごとに 1px or 2px の step が不均一」に見える (= 横揺れ感)。
/// 整数 snap すれば step は `1, 1, 2, 1, 1, 2 ...` 等の規則的パターンになり安定する。
fn sync_transform(mut query: Query<(&WorldPosition, &mut Transform), Changed<WorldPosition>>) {
    for (pos, mut transform) in &mut query {
        transform.translation =
            projection::world_to_bevy_f32(pos.x.round(), pos.y.round(), pos.z.round());
    }
}

/// 最終 flip_x = Facing flip XOR Animation 側 flip (frame.flip XOR layer.flip 済み)。
fn sync_flip(
    mut query: Query<
        (&Facing, &AnimationFrames, &mut Sprite),
        Or<(Changed<Facing>, Changed<AnimationFrames>)>,
    >,
) {
    for (facing, anim, mut sprite) in &mut query {
        sprite.flip_x = total_flip_x(*facing, anim.current_flip_x());
    }
}

/// `Facing` または `AnimationFrames`（frame 切替）が変わったら、現フレームの
/// base anchor を「最終 flip_x」に応じて x 反転して `Anchor` に書く。
fn sync_anchor(
    mut query: Query<
        (&AnimationFrames, &Facing, &mut Anchor),
        Or<(Changed<AnimationFrames>, Changed<Facing>)>,
    >,
) {
    for (anim, facing, mut anchor) in &mut query {
        let flip_x = total_flip_x(*facing, anim.current_flip_x());
        *anchor = flip_anchor(anim.current_anchor(), flip_x);
    }
}

/// Facing flip と Animation 側 flip を XOR する pure 関数。
#[must_use]
pub fn total_flip_x(facing: Facing, anim_flip_x: bool) -> bool {
    let facing_flip = matches!(facing, Facing::Left);
    facing_flip ^ anim_flip_x
}

/// `flip_x = true` のとき anchor.x の符号を反転する pure 関数。
#[must_use]
pub fn flip_anchor(base: Anchor, flip_x: bool) -> Anchor {
    if flip_x {
        Anchor(Vec2::new(-base.0.x, base.0.y))
    } else {
        base
    }
}

/// ADR-0038: 旧 `With<Player>` filter は「Hero side + Human controller」(= 旧 Player marker
/// と等価) で再表現する。複数 Hero+Human entity がある split-screen (ADR-0030 multi-player)
/// 想定では `single` ではなく PlayerId 別 camera が必要だが、Phase 1 の P1 only MVP
/// (= 旧挙動) を維持する。
fn camera_follow(
    project: Option<Res<Project>>,
    player: Query<(&WorldPosition, &Side, &Controller), Without<MainCamera>>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(project) = project else {
        return;
    };
    let Some(player_pos) = player.iter().find_map(|(pos, side, ctrl)| {
        (matches!(side, Side::Hero) && matches!(ctrl, Controller::Human)).then_some(pos)
    }) else {
        return;
    };
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let half_view = project.resolution.width as f32 / 2.0;
    // sync_transform と同じ理由で整数 snap。player.x は f32 のまま保持し、camera と
    // player 両方を同じ integer pixel に揃えることで両者の相対位置を sub-pixel ズレ
    // させない (沿わせないと sprite の nearest snap で 1px ぶれる)。
    transform.translation.x = clamp_camera_x(player_pos.x, half_view).round();
}

/// camera が world 左端を越えて左に行かないよう player の X を clamp する。
/// (右端 clamp は base 画像の幅を知る必要があるので別段階。)
#[must_use]
pub fn clamp_camera_x(player_x: f32, half_view_width: f32) -> f32 {
    player_x.max(half_view_width)
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // bit-exact 比較 (Default / new() / clamp の直接代入)
mod tests {
    use super::*;

    #[test]
    fn world_position_default_is_origin() {
        let p = WorldPosition::default();
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 0.0);
        assert_eq!(p.z, 0.0);
    }

    #[test]
    fn world_position_new_stores_components() {
        let p = WorldPosition::new(28.0, 0.0, 180.0);
        assert_eq!(p.x, 28.0);
        assert_eq!(p.y, 0.0);
        assert_eq!(p.z, 180.0);
    }

    #[test]
    fn facing_default_is_right() {
        assert_eq!(Facing::default(), Facing::Right);
    }

    #[test]
    fn clamp_camera_x_lets_camera_track_player_inside_view() {
        // player が view 半幅より右にいるときはそのまま追従
        assert_eq!(clamp_camera_x(500.0, 192.0), 500.0);
    }

    #[test]
    fn clamp_camera_x_pins_camera_at_left_edge() {
        // player が view 半幅より左 (world 左端付近) では camera を pin
        assert_eq!(clamp_camera_x(50.0, 192.0), 192.0);
        assert_eq!(clamp_camera_x(0.0, 192.0), 192.0);
    }

    #[test]
    fn flip_anchor_identity_when_not_flipped() {
        let base = Anchor(Vec2::new(0.12, -0.48));
        assert_eq!(flip_anchor(base, false), base);
    }

    #[test]
    fn flip_anchor_inverts_x_only_when_flipped() {
        let base = Anchor(Vec2::new(0.12, -0.48));
        let flipped = flip_anchor(base, true);
        assert_eq!(flipped.0.x, -0.12);
        assert_eq!(flipped.0.y, -0.48);
    }

    #[test]
    fn total_flip_x_xors_facing_and_animation() {
        assert!(!total_flip_x(Facing::Right, false));
        assert!(total_flip_x(Facing::Left, false));
        assert!(total_flip_x(Facing::Right, true));
        // 両方 flip だと打ち消し合って元に戻る
        assert!(!total_flip_x(Facing::Left, true));
    }

    #[test]
    fn step_axis_aware_moves_both_axes_when_unrestricted() {
        let pos = WorldPosition::new(10.0, 0.0, 20.0);
        let next = step_axis_aware(pos, 5.0, -3.0, |_, _| true);
        assert_eq!(next.x, 15.0);
        assert_eq!(next.z, 17.0);
    }

    #[test]
    fn step_axis_aware_drops_only_blocked_axis() {
        // x > 12 を outside、それ以外 inside。x 方向だけ拒否される。
        let pos = WorldPosition::new(10.0, 0.0, 20.0);
        let next = step_axis_aware(pos, 5.0, -3.0, |x, _| x <= 12.0);
        assert_eq!(next.x, 10.0);
        assert_eq!(next.z, 17.0);
    }

    #[test]
    fn step_axis_aware_holds_position_in_corner() {
        let pos = WorldPosition::new(10.0, 0.0, 20.0);
        let next = step_axis_aware(pos, 5.0, 5.0, |_, _| false);
        assert_eq!(next.x, 10.0);
        assert_eq!(next.z, 20.0);
    }
}
