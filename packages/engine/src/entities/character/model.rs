//! Character 集約の全型定義 (FSD: model segment)。
//!
//! editor-desktop の同名スライスとは独立で、engine 描画・再生に必要なフィールドのみを保持する。
//! editor 専用フィールド (`body_box_overrides` / `attack_box_overrides` / `export_number`) は
//! serde の未知フィールドとして silently ignore される。
//! ADR-0019 の Frame.sound と SoundGroup はこちらに保持する (engine 側で SE 再生する責務)。
//! ロード処理は隣の [`super::api`] に分離している。
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::shared::flip::FlipMode;

// === Defaults ===

pub const DEFAULT_DEPTH: u32 = 16;
pub const DEFAULT_HP: u32 = 100;

pub const DEFAULT_GRAVITY: f64 = 800.0;
pub const DEFAULT_JUMP_VELOCITY_Y: f64 = 200.0;
pub const DEFAULT_KNOCKBACK_THRESHOLD: u32 = 100;
pub const DEFAULT_BOUNCE_COUNT: u32 = 1;
pub const DEFAULT_BOUNCE_DAMPENING: f32 = 0.5;
pub const DEFAULT_GROUND_FRICTION: f64 = 600.0;
pub const DEFAULT_HIT_RECOVERY_MS: u32 = 1500;
pub const DEFAULT_LIE_DOWN_DURATION_MS: u32 = 800;
pub const DEFAULT_RISE_DURATION_MS: u32 = 300;
/// Guard ゲージの初期値 / max (ADR-0028)。`guard_damage` で削られて 0 以下になると GuardBreak 発動。
pub const DEFAULT_GUARD_BREAK_THRESHOLD: u32 = 100;
/// 最後にガード被弾してから何 ms で guard_gauge を full 回復するか (ADR-0028)。
pub const DEFAULT_GUARD_RECOVERY_MS: u32 = 1200;
/// 1 連続コンボあたりの空中再被弾 (= ジャグル) 最大回数。これを超えた airborne hit は
/// **完全無敵** (damage / state / gauge / consumed 全て不発) で素通りする (= 敵を当て続けても
/// 落下フローが進む。永久パターン回避)。
pub const DEFAULT_MAX_JUGGLE_COUNT: u32 = 3;
/// 1 連続コンボあたりの DownHit (= 倒れ中被弾) 最大回数。これを超えた down hit は
/// **完全無敵** (damage / state / gauge / consumed 全て不発) で素通りする (= 倒れたまま無敵、
/// 永久パターン回避)。
pub const DEFAULT_MAX_DOWN_HIT_COUNT: u32 = 3;

// === AI (ADR-0035) ===

/// `MeleeConfig` の default 値群。ADR-0035 の grunt YAML サンプルと同値で、Phase 1.1 で
/// `features/character/ai.rs` に hard-code していたものを entities に移植したもの。
pub const DEFAULT_AI_CHASE_ENTER_RANGE_PX: f32 = 200.0;
pub const DEFAULT_AI_CHASE_EXIT_RANGE_PX: f32 = 240.0;
pub const DEFAULT_AI_ATTACK_ENTER_RANGE_PX: f32 = 28.0;
pub const DEFAULT_AI_ATTACK_EXIT_RANGE_PX: f32 = 36.0;
pub const DEFAULT_AI_ATTACK_COOLDOWN_TICKS: u32 = 60;
pub const DEFAULT_AI_DECISION_INTERVAL_TICKS: u32 = 6;
pub const DEFAULT_AI_MIN_DWELL_TICKS: u32 = 8;

/// `AllyConfig` の Follow 系 default 値 (ADR-0035 Phase 2)。Player との距離が
/// `follow_distance_max` を超えたら Follow を再開し、`follow_distance_min` 未満で停止する。
/// hysteresis 幅は AI の 1 decision 周期で進む距離より十分大きく取り、境界振動を避ける。
pub const DEFAULT_AI_FOLLOW_DISTANCE_MIN_PX: f32 = 40.0;
pub const DEFAULT_AI_FOLLOW_DISTANCE_MAX_PX: f32 = 80.0;

// === Role ===

/// Animation の役割。engine 側 State (Idle/Walk/Attack/Hit/Jump/JumpAttack/Guard/GuardBreak +
/// 吹っ飛び flow) と semantic に紐付ける。役割なしの YAML や Custom Animation は
/// [`Role::Custom`] として扱う。
///
/// Knockback 系 (7 個 × 4 軸 = 通常 / Back / Dead / DeadBack の prefix; Rise は Dead 系 2 つを
/// 持たない) は ADR-0024/0025 の吹っ飛びフローに対応する。Animation 解決は
/// [`super::super::super::features::character::state_machine`] の `resolve_animation_role` が
/// `(state, hit_from_behind, final_action)` から 4 段フォールバック chain を試行する。
///
/// 旧 `dead` role は [`Role::DeadLieDown`] に集約 (serde alias で旧 YAML 互換)。
/// 旧 `block` role は [`Role::Guard`] に rename (ADR-0028、serde alias で旧 YAML 互換)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Idle,
    Walk,
    Attack,
    Hit,
    Jump,
    /// 空中攻撃 (ADR-0027)。Jump 中の `J` / `Space` で発動する Attack の独立 variant。
    JumpAttack,
    /// 立ちガード (ADR-0028)。旧 `block` role は serde alias でこの variant に読み替える。
    #[serde(alias = "block")]
    Guard,
    /// ガードクラッシュ (ADR-0028)。`guard_gauge <= 0` で 1〜数 frame 見せ、その後 KnockbackUp に
    /// 合流する中継 state 用の Animation Role。Back / Dead prefix variant は持たない (= 後続の
    /// Knockback フロー側で被弾方向 / 致命傷の prefix variant が選ばれるため、ここでは不要)。
    GuardBreak,
    KnockbackUp,
    KnockbackDown,
    BounceUp,
    BounceDown,
    Slide,
    LieDown,
    Rise,
    BackKnockbackUp,
    BackKnockbackDown,
    BackBounceUp,
    BackBounceDown,
    BackSlide,
    BackLieDown,
    BackRise,
    DeadKnockbackUp,
    DeadKnockbackDown,
    DeadBounceUp,
    DeadBounceDown,
    DeadSlide,
    /// 死亡時の最終静止。Rise に進まず Animation 末尾で永続停止する。
    /// 旧形式 (`role: dead`) は alias でこの variant に読み替えられる。
    #[serde(alias = "dead")]
    DeadLieDown,
    DeadBackKnockbackUp,
    DeadBackKnockbackDown,
    DeadBackBounceUp,
    DeadBackBounceDown,
    DeadBackSlide,
    DeadBackLieDown,
    /// Down 中 (Slide / LieDown / Rise) に被弾したときの地上 hit ポーズ。Hit が立ちポーズ
    /// 前提なので、地面に伏せている被弾には別 role を割り当てる。
    DownHit,
    /// 下段攻撃ポーズ (`K` キー)。AttackBox を低位置に置いて倒れた敵 (LieDown) に当てる用。
    DownAttack,
    #[default]
    Custom,
}

impl Role {
    /// battle scene が起動時にロードを試みる Role 一覧 (Custom 以外)。順序は安定性のため
    /// 「基本 → Knockback (通常) → Back → Dead → DeadBack」で固定する。
    #[must_use]
    pub const fn all_loadable() -> &'static [Role] {
        &[
            Role::Idle,
            Role::Walk,
            Role::Attack,
            Role::Hit,
            Role::Jump,
            Role::JumpAttack,
            Role::Guard,
            Role::GuardBreak,
            Role::KnockbackUp,
            Role::KnockbackDown,
            Role::BounceUp,
            Role::BounceDown,
            Role::Slide,
            Role::LieDown,
            Role::Rise,
            Role::BackKnockbackUp,
            Role::BackKnockbackDown,
            Role::BackBounceUp,
            Role::BackBounceDown,
            Role::BackSlide,
            Role::BackLieDown,
            Role::BackRise,
            Role::DeadKnockbackUp,
            Role::DeadKnockbackDown,
            Role::DeadBounceUp,
            Role::DeadBounceDown,
            Role::DeadSlide,
            Role::DeadLieDown,
            Role::DeadBackKnockbackUp,
            Role::DeadBackKnockbackDown,
            Role::DeadBackBounceUp,
            Role::DeadBackBounceDown,
            Role::DeadBackSlide,
            Role::DeadBackLieDown,
            Role::DownHit,
            Role::DownAttack,
        ]
    }
}

// === Physics ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Physics {
    #[serde(default)]
    pub gravity: f64,
    #[serde(default)]
    pub jump_velocity_y: f64,
    #[serde(default)]
    pub knockback_threshold: u32,
    #[serde(default)]
    pub knockback_resistance: f32,
    #[serde(default)]
    pub bounce_count: u32,
    #[serde(default)]
    pub bounce_dampening: f32,
    #[serde(default)]
    pub ground_friction: f64,
    #[serde(default)]
    pub hit_recovery_ms: u32,
    #[serde(default)]
    pub lie_down_duration_ms: u32,
    #[serde(default)]
    pub rise_duration_ms: u32,
    /// 1 連続コンボあたりの空中再被弾 (ジャグル) 最大回数。`Combatant.juggle_count` がこれを
    /// 超えた airborne hit は **完全無敵** (damage / state / gauge / consumed 全て不発、
    /// AABB ヒットしても素通り) になる (= 永久パターン回避)。
    /// Rise → Idle で counter は reset される。
    #[serde(default)]
    pub max_juggle_count: u32,
    /// 1 連続コンボあたりの DownHit 最大回数。`Combatant.down_hit_count` がこれを超えた
    /// down hit は **完全無敵** (damage / state / gauge / consumed 全て不発、AABB ヒットしても
    /// 素通り) になる (= 倒れたまま無敵、永久パターン回避)。
    /// Rise → Idle で counter は reset される。
    #[serde(default)]
    pub max_down_hit_count: u32,
    /// Guard ゲージの初期値 / max (ADR-0028)。`AttackBoxMeta.guard_damage` で削られて
    /// 0 以下になると GuardBreak 発動。
    #[serde(default)]
    pub guard_break_threshold: u32,
    /// 最後にガード被弾してから何 ms で `guard_gauge` を full 回復するか (ADR-0028)。
    /// `hit_recovery_ms` と同型の自然回復モデル。
    #[serde(default)]
    pub guard_recovery_ms: u32,
    /// GuardBreak 発動時に被弾側へ充填する吹っ飛びベクトル (ADR-0028)。
    /// `KnockbackVec` と同じく `vel_x` は「攻撃側前方 = +」基準で書き、scene 側で Facing 反転する。
    #[serde(default)]
    pub guard_break_knockback: KnockbackVec,
}

impl Default for Physics {
    fn default() -> Self {
        Self {
            gravity: DEFAULT_GRAVITY,
            jump_velocity_y: DEFAULT_JUMP_VELOCITY_Y,
            knockback_threshold: DEFAULT_KNOCKBACK_THRESHOLD,
            knockback_resistance: 0.0,
            bounce_count: DEFAULT_BOUNCE_COUNT,
            bounce_dampening: DEFAULT_BOUNCE_DAMPENING,
            ground_friction: DEFAULT_GROUND_FRICTION,
            hit_recovery_ms: DEFAULT_HIT_RECOVERY_MS,
            lie_down_duration_ms: DEFAULT_LIE_DOWN_DURATION_MS,
            rise_duration_ms: DEFAULT_RISE_DURATION_MS,
            max_juggle_count: DEFAULT_MAX_JUGGLE_COUNT,
            max_down_hit_count: DEFAULT_MAX_DOWN_HIT_COUNT,
            guard_break_threshold: DEFAULT_GUARD_BREAK_THRESHOLD,
            guard_recovery_ms: DEFAULT_GUARD_RECOVERY_MS,
            // GuardBreak 既定: knockback と同等の弱めの吹っ飛びを起こして、ADR-0024 のフローへ。
            guard_break_knockback: KnockbackVec {
                vel_x: 100.0,
                vel_y: 150.0,
                vel_z: 0.0,
            },
        }
    }
}

// === AiConfig (ADR-0035) ===

/// Character YAML の `ai:` セクション。`kind` で AI Brain の種類を切り替える
/// internally-tagged enum (ADR-0035)。Phase 1 は `Melee` のみ、Phase 2 で `Ally` (味方 NPC)
/// を追加、Phase 4 (ADR-0038) で `Bot` (Player 自動化) を YAML 化。将来 `Ranged` 等を増やす。
/// Hero character は `ai: null` (= 未指定) で `Controller::Human` の手動操作を維持し、
/// `ai: kind: bot` を書くと `Controller::Ai` の自動化に切り替える。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiConfig {
    Melee(MeleeConfig),
    /// ADR-0035 Phase 2: 味方 NPC 用 Brain。Player に追従しつつ、検出範囲内の Enemy を
    /// 殴りに行く。target 選定は「最も近い Enemy 優先、不在時は Player follow」。
    Ally(AllyConfig),
    /// ADR-0035 Phase 3 / ADR-0038 Phase 4: Player 自動化 Brain。env var
    /// `BEATEMUP_PLAYER_BOT` の YAML 版で、Hero character YAML に `ai: kind: bot` を書くと
    /// `Controller::Ai` + `BotBrain` で spawn される。env var が指定されている場合は env var
    /// 優先 (= 既存挙動の回帰なし、ADR-0035 Phase 3 補追の規約を維持)。
    Bot(BotConfig),
}

/// Brain が毎 tick で **次 target を選ぶ戦略** (ADR-0039)。各 Brain (Melee/Ally/Bot) の Config に
/// 1 つずつ持たせ、`select_target` helper が enum 値に応じてディスパッチする。
///
/// - `Nearest`: 候補集合の中で **距離最小** を選ぶ (Phase 1.1 から既存の hardcode 動作)。
/// - `LastEngaged`: 前回 tick で engage した target が生存 + side 一致 なら **継続追跡**。
///   ロスト時は `Nearest` にフォールバック (= ADR-0035 Phase 2 補追で浮上した Ally の継続追跡要望
///   への解)。
/// - `Random` / `WeightedByThreat`: variant 定義 + stub。tick 時に該当 selector が呼ばれたら
///   warn を 1 回だけ吐いて `Nearest` フォールバックする (= YAML 互換性を将来までキープ)。
///   実装は実需 (Random demo / ボス級 hate 管理) が出た時点で別 ADR + Issue で行う。
///
/// 既存 YAML が `selector` 未指定でも default = `Nearest` で挙動 bit-exact 互換。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSelector {
    #[default]
    Nearest,
    LastEngaged,
    Random,
    WeightedByThreat,
}

/// Idle/Chase/Attack の FSM 遷移と Brain tick 周期に関する共通パラメータ (ADR-0039)。
/// `MeleeConfig` / `AllyConfig` / `BotConfig` の 3 Brain で同形の field 群を 1 サブ構造に集約し、
/// `#[serde(flatten)]` で各 BrainConfig の YAML 表現に展開する (= YAML 上は flat field のまま、
/// Rust 側は `cfg.engagement.*` で参照する)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngagementConfig {
    /// Chase に入る距離。`chase_enter < chase_exit` で hysteresis を作る。
    #[serde(default = "default_ai_chase_enter_range_px")]
    pub chase_enter_range_px: f32,
    /// Chase から Idle に戻る距離。
    #[serde(default = "default_ai_chase_exit_range_px")]
    pub chase_exit_range_px: f32,
    /// Attack 発火距離。`attack_enter < attack_exit` で hysteresis を作る。
    #[serde(default = "default_ai_attack_enter_range_px")]
    pub attack_enter_range_px: f32,
    /// Attack から Chase に戻る距離。
    #[serde(default = "default_ai_attack_exit_range_px")]
    pub attack_exit_range_px: f32,
    /// 攻撃発火後の cooldown (この間 Brain は `attack: false` を返す)。
    #[serde(default = "default_ai_attack_cooldown_ticks")]
    pub attack_cooldown_ticks: u32,
    /// N frame ごとに decision を回す。1 = 毎 frame。
    #[serde(default = "default_ai_decision_interval_ticks")]
    pub decision_interval_ticks: u32,
    /// state 遷移後の最低滞在 tick (チャタリング防止 軸 2)。
    #[serde(default = "default_ai_min_dwell_ticks")]
    pub min_dwell_ticks: u32,
}

impl Default for EngagementConfig {
    fn default() -> Self {
        Self {
            chase_enter_range_px: DEFAULT_AI_CHASE_ENTER_RANGE_PX,
            chase_exit_range_px: DEFAULT_AI_CHASE_EXIT_RANGE_PX,
            attack_enter_range_px: DEFAULT_AI_ATTACK_ENTER_RANGE_PX,
            attack_exit_range_px: DEFAULT_AI_ATTACK_EXIT_RANGE_PX,
            attack_cooldown_ticks: DEFAULT_AI_ATTACK_COOLDOWN_TICKS,
            decision_interval_ticks: DEFAULT_AI_DECISION_INTERVAL_TICKS,
            min_dwell_ticks: DEFAULT_AI_MIN_DWELL_TICKS,
        }
    }
}

/// `MeleeBrain` (近接 AI) のパラメータ。range は `_enter`/`_exit` 2 段のヒステリシスで
/// 境界振動を構造的に吸収する (ADR-0035 チャタリング防止 軸 1)。`Default` は
/// `DEFAULT_AI_*` 定数群と同値。
///
/// 単純な data type (Component ではない)。実体は `features::character::ai::MeleeBrain.config`
/// が保持し、Brain の意思決定パラメータとして使われる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MeleeConfig {
    /// 共通 FSM パラメータ。YAML 上は flat field (`chase_enter_range_px` 等) として展開される
    /// (ADR-0039、`#[serde(flatten)]`)。
    #[serde(flatten)]
    pub engagement: EngagementConfig,
    /// target 選定戦略 (ADR-0039)。未指定 = `Nearest` で Phase 1.1 既存挙動互換。
    #[serde(default)]
    pub selector: TargetSelector,
}

/// `AllyBrain` (味方 NPC AI) のパラメータ。Chase / Attack の hysteresis は `EngagementConfig`
/// と同じ枠組みで、追加で Player との Follow 距離 hysteresis を持つ (ADR-0035 Phase 2)。
///
/// 単純な data type (Component ではない)。実体は `features::character::ai::AllyBrain.config`
/// が保持し、Brain の意思決定パラメータとして使われる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllyConfig {
    /// 共通 FSM パラメータ (`#[serde(flatten)]` で YAML 上は flat field として展開される)。
    #[serde(flatten)]
    pub engagement: EngagementConfig,
    /// Player との距離がこれ未満で Follow を停止 (= Idle 相当、move=0)。
    #[serde(default = "default_ai_follow_distance_min_px")]
    pub follow_distance_min_px: f32,
    /// Player との距離がこれ超で Follow 再開。`min < max` で hysteresis を作る。
    #[serde(default = "default_ai_follow_distance_max_px")]
    pub follow_distance_max_px: f32,
    /// target 選定戦略 (ADR-0039)。Ally engagement (Villain target) に作用する。
    /// Follow target (= 最近の Hero+Human) は selector 適用対象外で常に Nearest。
    #[serde(default)]
    pub selector: TargetSelector,
}

const fn default_ai_follow_distance_min_px() -> f32 {
    DEFAULT_AI_FOLLOW_DISTANCE_MIN_PX
}
const fn default_ai_follow_distance_max_px() -> f32 {
    DEFAULT_AI_FOLLOW_DISTANCE_MAX_PX
}

const fn default_ai_chase_enter_range_px() -> f32 {
    DEFAULT_AI_CHASE_ENTER_RANGE_PX
}
const fn default_ai_chase_exit_range_px() -> f32 {
    DEFAULT_AI_CHASE_EXIT_RANGE_PX
}
const fn default_ai_attack_enter_range_px() -> f32 {
    DEFAULT_AI_ATTACK_ENTER_RANGE_PX
}
const fn default_ai_attack_exit_range_px() -> f32 {
    DEFAULT_AI_ATTACK_EXIT_RANGE_PX
}
const fn default_ai_attack_cooldown_ticks() -> u32 {
    DEFAULT_AI_ATTACK_COOLDOWN_TICKS
}
const fn default_ai_decision_interval_ticks() -> u32 {
    DEFAULT_AI_DECISION_INTERVAL_TICKS
}
const fn default_ai_min_dwell_ticks() -> u32 {
    DEFAULT_AI_MIN_DWELL_TICKS
}

impl Default for AllyConfig {
    fn default() -> Self {
        Self {
            engagement: EngagementConfig::default(),
            follow_distance_min_px: DEFAULT_AI_FOLLOW_DISTANCE_MIN_PX,
            follow_distance_max_px: DEFAULT_AI_FOLLOW_DISTANCE_MAX_PX,
            selector: TargetSelector::default(),
        }
    }
}

/// `BotBrain` (Player 自動化 AI) のパラメータ (ADR-0038 Phase 4)。
/// Phase 3 では `MeleeConfig` を流用していたが、YAML 化に伴い `Bot` 専用の Config 型を
/// 切る。フィールド構成は `EngagementConfig` 共有のみ (= 雑魚 Enemy と同調律で動く)。
/// Bot 専用 param (perception / panic / replay 等) が必要になったら本 struct に追加する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BotConfig {
    /// 共通 FSM パラメータ (`#[serde(flatten)]` で YAML 上は flat field として展開される)。
    #[serde(flatten)]
    pub engagement: EngagementConfig,
    /// target 選定戦略 (ADR-0039)。未指定 = `Nearest` で Phase 3 既存挙動互換。
    #[serde(default)]
    pub selector: TargetSelector,
}

impl BotConfig {
    /// `BotConfig` を `MeleeConfig` に詰め替える (`BotBrain::new` が `MeleeConfig` を要求する
    /// ため、scene spawn 時に変換する)。Phase 4 では「BotBrain の挙動 = MeleeBrain と同形」
    /// 規約を維持しており、`BotConfig` は MeleeConfig の YAML 版という位置付け。
    #[must_use]
    pub fn into_melee_config(self) -> MeleeConfig {
        MeleeConfig {
            engagement: self.engagement,
            selector: self.selector,
        }
    }
}

// === Character ===

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Character {
    /// YAML に `name` が無ければ file stem を [`super::api`] 側で埋める。
    #[serde(default)]
    pub name: String,
    /// editor が生成する thumbnail への、character ディレクトリ起点の相対パス。
    /// 実体は `runtime/data/characters/{name}/{thumbnail_path}`。AssetServer 経由でロード可能。
    #[serde(default)]
    pub thumbnail_path: String,
    #[serde(default = "default_hp")]
    pub hp: u32,
    #[serde(default = "default_depth")]
    pub depth: u32,
    /// ADR-0031: HUD の `enemy_hp_bar` 等が `target: { tag: "boss" }` で識別するのに使う任意ラベル。
    /// Player には影響しない。複数キャラに同じ tag を付けたときは "最初に生成された entity" が選ばれる。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default)]
    pub physics: Physics,
    /// AI Brain の設定 (ADR-0035)。`None` (YAML 未指定) なら AI Brain を attach しない
    /// (= Player か、行動しない object として扱う)。`Some(Melee(cfg))` のときは scene 側が
    /// `MeleeBrain::new(cfg)` を Enemy entity に attach する。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiConfig>,
    /// `runtime/data/characters/{name}/sprite-groups/*.yml` から populate される。
    /// key は `SpriteGroup.number` (= Layer.sprite_group_number から参照)。
    /// YAML には書かれない。
    #[serde(skip)]
    pub sprite_groups: HashMap<u32, SpriteGroup>,
    /// `runtime/data/characters/{name}/animations/*.yml` から populate される。
    /// YAML には書かれない。
    #[serde(skip)]
    pub animations: Vec<Animation>,
    /// `runtime/data/characters/{name}/sound-groups/*.yml` から populate される (ADR-0019)。
    /// key は `SoundGroup.number` (= `Frame.sound.number` から参照)。YAML には書かれない。
    #[serde(skip)]
    pub sound_groups: HashMap<u32, SoundGroup>,
}

impl Character {
    /// `Frame.sound.number` から `SoundGroup` を引く (ADR-0019)。見つからなければ `None`。
    /// engine の SE dispatch system がこの helper 経由で SoundGroup を解決する。
    #[must_use]
    pub fn find_sound_group(&self, number: u32) -> Option<&SoundGroup> {
        self.sound_groups.get(&number)
    }
}

// === SpriteGroup / SpriteEntry ===

/// `runtime/data/characters/{character}/sprite-groups/{group}.yml` の構造。
/// engine 描画では `pivot_point` だけが必須なので、body_boxes / attack_boxes は
/// serde の未知フィールドとして silently ignore される。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpriteGroup {
    /// YAML には書かれず、loader (api 側) が file stem で埋める。
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub sprites: Vec<SpriteEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpriteEntry {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub path: String,
    /// 画像内のキャラ pivot 位置 (ピクセル, [x, y])。
    /// Go 版 engine の `layer_origin = char_pos + (-sprite.pivot_point) + ...` で
    /// 「画像のここを char_pos に重ねる」基準点。
    #[serde(default)]
    pub pivot_point: [i32; 2],
    /// 被弾判定 box の **メイン**。Frame.body_box_overrides が None (Inherit) のときは
    /// この値が使われる。editor 側の Sprite.body_boxes と同じ schema。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_boxes: Option<Vec<HitBox>>,
    /// 攻撃判定 box の **メイン**。Frame.attack_box_overrides が None (Inherit) のときは
    /// この値が使われる。editor 側の Sprite.attack_boxes と同じ schema。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_boxes: Option<Vec<AttackBox>>,
}

// === Animation / Frame / Layer ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Animation {
    /// YAML には書かれず、loader (api 側) が file stem で埋める。
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub role: Role,
    /// Multi-cardinality role (Attack/Hit/Dead/Jump) の役割内 slot 番号 (0-indexed)。
    /// Single-cardinality role では 0 固定。
    #[serde(default)]
    pub variant: u32,
    #[serde(default)]
    pub is_loop: bool,
    #[serde(default)]
    pub loop_start_index: u32,
    #[serde(default)]
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Frame {
    #[serde(default)]
    pub index: u32,
    /// 60Hz vsync tick (= 1/60 秒) 単位。例: `7` で 7 tick = 約 116.67 ms。
    /// 0 のときは Default フレーム長 (engine 側で別途決める) を使うことを想定。
    /// **データモデル上の唯一の時間単位**で、engine / editor / animation export
    /// 全てがこの tick を 1 級概念として扱う (ms 概念は持ち込まない)。
    #[serde(default)]
    pub ticks: u32,
    /// frame レベルの反転 (frame 全体を反転)。`null` で反転なし。
    #[serde(default)]
    pub flip: Option<FlipMode>,
    /// `null` のとき (0, 0)。`Some([dx, dy])` のとき `dx`, `dy` が pivot に加算される。
    #[serde(default)]
    pub pivot_point_offset: Option<[i32; 2]>,
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// この frame で active な攻撃判定 box の上書き列。editor 側の 3-state と互換:
    /// `None`=Inherit (上位 SpriteEntry.attack_boxes に従う)、`Some(empty)`=Disable
    /// (攻撃判定なし)、`Some(non-empty)`=Override (この frame の box を使う)。
    /// 各 [`AttackBoxOverride`] 要素は hitbox / meta を個別に Option で持ち、None の field は
    /// sprite 側の同じ index の要素から継承する (`battle::resolve_attack_box` で merge)。
    #[serde(default)]
    pub attack_box_overrides: Option<Vec<AttackBoxOverride>>,
    /// この frame で active な被弾判定 box の上書き列。`attack_box_overrides` と同じ
    /// 3-state で、`None`=Inherit (`SpriteEntry.body_boxes` を継承)。
    #[serde(default)]
    pub body_box_overrides: Option<Vec<HitBox>>,
    /// この frame に進入した瞬間に発火する Sound 参照 (ADR-0019)。`None` で無音。
    /// `number` は `Character.sound_groups` の key、`delay_ms` は frame 進入から
    /// 再生開始までの遅延 (ms)。0 で frame 進入と同 tick に再生する。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<FrameSound>,
}

// === AttackBox / HitBox / AttackBoxMeta / KnockbackVec ===
// editor 側 (packages/editor-desktop/src/shared/collision.rs) と YAML 上互換になるよう
// フィールド名・形を揃える。engine は読み取り専用なので resize 系 helper は持たない。

/// `AttackBoxMeta.knockback` が保持する吹っ飛び速度ベクトル。
/// `vel_x` の符号は「攻撃側の前方向 = +」(scene 側で `Facing` を見て符号反転する)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct KnockbackVec {
    #[serde(default)]
    pub vel_x: f32,
    #[serde(default)]
    pub vel_y: f32,
    #[serde(default)]
    pub vel_z: f32,
}

/// 攻撃の効果データ。geometry (HitBox) と分離して、ダメージ / Knockback ゲージ削り /
/// 吹っ飛びベクトル / hit_stop 演出を表す (ADR-0024)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AttackBoxMeta {
    pub damage: u32,
    pub knockback_damage: u32,
    pub knockback: KnockbackVec,
    /// Guard 中の被弾で削る guard_gauge 量 (ADR-0028)。
    /// 0 でも Guard 中の damage / knockback_gauge は無効化される (= ガード成立中は無傷)。
    /// 「ガード不能」を表したい場合は将来 `guard_break_only: bool` 等で別途表現する。
    pub guard_damage: u32,
    /// hit が決まった瞬間に発生する time freeze + sprite 揺らし演出。`None` で hit_stop なし
    /// (= 即座に通常の Hit state へ遷移)。詳細は [`HitStop`]。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hit_stop: Option<HitStop>,
}

/// hit 瞬間の time freeze + visual shake 演出パラメータ。攻撃側 frame の meta に置く
/// (= 攻撃の重さ・性質で決まる)。被弾側 sprite には三角波の shake が visual offset と
/// して乗り、その間 attacker / victim 両方の Animation 進行と CharacterState 遷移が
/// freeze される。world position は不動。
///
/// 軸の取り方:
/// - `shake_x`: キャラ向きの **前方** が +、後方が - (world X)。1 片道目の方向。
/// - `shake_y`: 画面上が +、画面下が - (world Y)。1 片道目の方向。
///
/// 三角波: `count` = 片道回数 (= 中心 ↔ ±max を 1 と数える)。1 = 中心 → +max で終了
/// (= 旧 impact 単発相当)、2 = 中心 → +max → 中心、4 = 1 周期 (中心 → +max → 中心 →
/// -max → 中心)。`decay` で振幅を線形に減衰させる。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HitStop {
    /// hit_stop の継続時間 (ms)。`None` のときは被弾側 Hit アニメ frame 0 の duration が
    /// そのまま使われる (= 被弾側固有値、ザコ vs ボス で揺れ時間を変えたい場合用の fallback)。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    /// shake の初期振幅 (px)。三角波の中心は 0、振幅は ±shake_x / ±shake_y。
    pub shake_x: i32,
    pub shake_y: i32,
    /// 片道回数。0 で shake なし。
    pub count: u32,
    /// shake 振幅の線形減衰率。`amplitude(progress) = shake * (1 - decay * progress).clamp(0, 1)`。
    /// 0.0 で振幅一定、1.0 で末尾の振幅 0。
    pub decay: f32,
}

/// 画像 pixel 座標で表された矩形 + 奥行き厚み (world Z)。
/// `top_left` / `bottom_right` は sprite 画像内ローカル座標、`depth` は world Z の全幅。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitBox {
    pub top_left: [i32; 2],
    pub bottom_right: [i32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

/// AttackBox = HitBox (幾何) + AttackBoxMeta (効果)。editor が serialize する新形式
/// (`{ hitbox: {...}, meta: {...} }`) を読む。旧形式 (HitBox 直接) は editor 側で
/// 新形式に migrate される前提で、engine 側では新形式のみ受ける。
/// sprite 側 (`SpriteEntry.attack_boxes`) のソース。`hitbox` は必須、`meta` のみ optional。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttackBox {
    pub hitbox: HitBox,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<AttackBoxMeta>,
}

/// `Frame.attack_box_overrides` の各要素。`hitbox` / `meta` を個別に Option で持ち、None の
/// field は sprite 側 (`SpriteEntry.attack_boxes`) の同じ index の要素から継承する。
/// 両方 Some なら sprite を完全に上書き、両方 None なら何もしない (= sprite をそのまま使う)。
/// YAML 互換: 既存 editor が書く `{ hitbox: {...}, meta: {...} }` の形 (両方 Some) は
/// そのまま deserialize できる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AttackBoxOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hitbox: Option<HitBox>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<AttackBoxMeta>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Layer {
    #[serde(default)]
    pub index: u32,
    /// 参照する `SpriteGroup` を **number** で指定する (name のリネーム耐性)。
    #[serde(default)]
    pub sprite_group_number: u32,
    /// SpriteGroup 内の Sprite を **index** で指定する (filename 変更耐性)。
    #[serde(default)]
    pub sprite_index: u32,
    /// 0.0 〜 1.0 の透明度。1.0 で完全不透明。
    #[serde(default = "default_transparency")]
    pub transparency: f32,
    /// layer レベルの反転 (frame 内でこの layer のみを反転)。`null` で反転なし。
    /// 最終 flip = facing XOR (frame.flip XOR layer.flip)。
    #[serde(default)]
    pub flip: Option<FlipMode>,
    #[serde(default)]
    pub pivot_point_offset: Option<[i32; 2]>,
}

impl Frame {
    /// `pivot_point_offset` を `(x, y)` で取り出す。`None` のときは (0, 0)。
    #[must_use]
    pub fn pivot_offset_xy(&self) -> (i32, i32) {
        offset_xy(self.pivot_point_offset)
    }
}

impl Layer {
    /// `pivot_point_offset` を `(x, y)` で取り出す。`None` のときは (0, 0)。
    #[must_use]
    pub fn pivot_offset_xy(&self) -> (i32, i32) {
        offset_xy(self.pivot_point_offset)
    }
}

// === Sound / SoundGroup / FrameSound (ADR-0019) ===

/// 同じ用途の音 (pain / death / 攻撃ボイス 等) をまとめた集合。`number` は YAML に書く
/// 識別子で、`Frame.sound.number` から参照される。複数 `Sound` を持てば `pick` で
/// `weight` 付きランダムに 1 つ選ばれる。editor 側の同名型と YAML 上互換。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SoundGroup {
    /// YAML には書かれず、loader (api 側) が file stem で埋める。
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub sounds: Vec<Sound>,
}

/// 1 つの音源ファイル + ボリューム + 抽選重み (ADR-0019)。`weight` は省略 / 0 / 負値で
/// 1.0 にフォールバックされる (= 全 Sound 等確率)。明示的に重みを書いた時だけ偏らせる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Sound {
    #[serde(default)]
    pub index: u32,
    /// `runtime/data/characters/{character}/sound-groups/{group}/sounds/` 起点の相対 path。
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub volume: f32,
    #[serde(default)]
    pub weight: f32,
}

/// Frame に紐づく Sound 参照 + 再生遅延 (ADR-0019 / ADR-0034)。
///
/// 3 系統の SoundGroup 参照で attacker 側 attack 結果ごとに出し分ける:
/// - `number`: 既定 (= 攻撃の振り音 / Hit voice / 通常時セリフ等、無条件で frame 進入時に
///   latch したい音全般)。`AttackOutcome::Idle` 時、または on_hit/on_guard が None 時の
///   フォールバック先
/// - `on_hit`: 直近 AttackBox が Hit したときに優先 latch
/// - `on_guard`: 直近 AttackBox が Guard されたときに優先 latch
///
/// `delay_ms` は 3 系統共通で frame 進入から再生開始までの遅延 (ms)。
/// engine の dispatch tick = `VSYNC_TICK` (≈16.667ms) なので実効分解能は 1 tick 単位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FrameSound {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_hit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_guard: Option<u32>,
    #[serde(default)]
    pub delay_ms: u32,
}

impl SoundGroup {
    /// 重み付きランダムで `Sound` を 1 つ選ぶ (ADR-0019)。`rand` は `f32 ∈ [0, 1)` を返す
    /// closure (テストで決定的にできる)。
    ///
    /// 規約:
    /// - 空 group は `None`
    /// - `weight <= 0.0` は 1.0 として扱う (= 等確率 fallback)
    /// - 累積比較が浮動小数誤差で末尾を抜けたら末尾要素を返す
    pub fn pick<F: FnOnce() -> f32>(&self, rand: F) -> Option<&Sound> {
        if self.sounds.is_empty() {
            return None;
        }
        let total: f32 = self.sounds.iter().map(|s| effective_weight(s.weight)).sum();
        if total <= 0.0 {
            // 全要素が weight 不正 (NaN 等)。先頭で safety net。
            return self.sounds.first();
        }
        let mut roll = rand() * total;
        for s in &self.sounds {
            roll -= effective_weight(s.weight);
            if roll <= 0.0 {
                return Some(s);
            }
        }
        // 累積比較で末尾を抜けた (浮動小数誤差) 場合の fallback
        self.sounds.last()
    }
}

/// `weight <= 0.0` (NaN 含む) を 1.0 に倒す。ADR-0019 の「省略時 = 等確率」を表現する。
fn effective_weight(w: f32) -> f32 {
    if w > 0.0 { w } else { 1.0 }
}

// === helpers ===

fn default_hp() -> u32 {
    DEFAULT_HP
}

fn default_depth() -> u32 {
    DEFAULT_DEPTH
}

fn default_transparency() -> f32 {
    1.0
}

fn offset_xy(opt: Option<[i32; 2]>) -> (i32, i32) {
    opt.map_or((0, 0), |a| (a[0], a[1]))
}

// === tests ===

#[cfg(test)]
mod tests {
    // ADR-0035 Phase 2: 新規追加した AiConfig::Ally の round-trip test で
    // refutable let-else の fallback panic! を使うため。
    #![allow(clippy::panic)]
    use super::*;

    // Physics::default は DEFAULT_* 定数をそのまま代入しているので bit-exact 一致を期待する。
    #[test]
    #[allow(clippy::float_cmp)]
    fn physics_default_matches_engine_constants() {
        let p = Physics::default();
        assert_eq!(p.gravity, DEFAULT_GRAVITY);
        assert_eq!(p.jump_velocity_y, DEFAULT_JUMP_VELOCITY_Y);
        assert_eq!(p.knockback_threshold, DEFAULT_KNOCKBACK_THRESHOLD);
        assert_eq!(p.knockback_resistance, 0.0);
        assert_eq!(p.bounce_count, DEFAULT_BOUNCE_COUNT);
        assert_eq!(p.bounce_dampening, DEFAULT_BOUNCE_DAMPENING);
        assert_eq!(p.ground_friction, DEFAULT_GROUND_FRICTION);
        assert_eq!(p.hit_recovery_ms, DEFAULT_HIT_RECOVERY_MS);
        assert_eq!(p.lie_down_duration_ms, DEFAULT_LIE_DOWN_DURATION_MS);
        assert_eq!(p.rise_duration_ms, DEFAULT_RISE_DURATION_MS);
        assert_eq!(p.max_juggle_count, DEFAULT_MAX_JUGGLE_COUNT);
        assert_eq!(p.max_down_hit_count, DEFAULT_MAX_DOWN_HIT_COUNT);
        assert_eq!(p.guard_break_threshold, DEFAULT_GUARD_BREAK_THRESHOLD);
        assert_eq!(p.guard_recovery_ms, DEFAULT_GUARD_RECOVERY_MS);
        assert_eq!(p.guard_break_knockback.vel_x, 100.0);
        assert_eq!(p.guard_break_knockback.vel_y, 150.0);
        assert_eq!(p.guard_break_knockback.vel_z, 0.0);
    }

    #[test]
    fn role_block_yaml_alias_reads_as_guard() {
        // 旧 YAML (`role: block`) が新 `Role::Guard` に読み替えられること (ADR-0028 互換)。
        let role: Role = serde_saphyr::from_str("block").expect("alias should parse");
        assert_eq!(role, Role::Guard);
    }

    #[test]
    fn frame_pivot_offset_xy_none_returns_zero() {
        let f = Frame::default();
        assert_eq!(f.pivot_offset_xy(), (0, 0));
    }

    #[test]
    fn frame_pivot_offset_xy_some_returns_values() {
        let f = Frame {
            pivot_point_offset: Some([4, -7]),
            ..Frame::default()
        };
        assert_eq!(f.pivot_offset_xy(), (4, -7));
    }

    #[test]
    fn layer_pivot_offset_xy_none_returns_zero() {
        let l = Layer::default();
        assert_eq!(l.pivot_offset_xy(), (0, 0));
    }

    #[test]
    fn layer_default_transparency_is_zero_via_struct_default() {
        // Default::default() は #[serde(default = "default_transparency")] を経由しない
        // ので f32::default() = 0.0 になる点に注意。YAML 由来は 1.0 に倒れる
        // (test は YAML 経由のロード後で見る、ここでは struct default 値を文書化)。
        let l = Layer::default();
        assert!((l.transparency - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn animation_default_role_is_custom() {
        let a = Animation::default();
        assert_eq!(a.role, Role::Custom);
    }

    #[test]
    fn hit_stop_default_is_zero_offsets() {
        let hs = HitStop::default();
        assert_eq!(hs.duration_ms, None);
        assert_eq!(hs.shake_x, 0);
        assert_eq!(hs.shake_y, 0);
        assert_eq!(hs.count, 0);
        assert!((hs.decay - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn attack_box_meta_with_hit_stop_round_trip() -> anyhow::Result<()> {
        let yaml = r"
damage: 30
hit_stop:
  duration_ms: 120
  shake_x: 2
  shake_y: 4
  count: 3
  decay: 0.5
";
        let meta: AttackBoxMeta = serde_saphyr::from_str(yaml)?;
        assert_eq!(meta.damage, 30);
        let hs = meta.hit_stop.expect("hit_stop should be present");
        assert_eq!(hs.duration_ms, Some(120));
        assert_eq!(hs.shake_x, 2);
        assert_eq!(hs.shake_y, 4);
        assert_eq!(hs.count, 3);
        assert!((hs.decay - 0.5).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn attack_box_meta_without_hit_stop_field_yields_none() -> anyhow::Result<()> {
        // 既存 YAML (hit_stop なし) は meta.hit_stop = None で互換的に読める。
        let yaml = r"
damage: 30
knockback_damage: 5
";
        let meta: AttackBoxMeta = serde_saphyr::from_str(yaml)?;
        assert_eq!(meta.damage, 30);
        assert!(meta.hit_stop.is_none());
        Ok(())
    }

    #[test]
    fn hit_stop_without_duration_ms_field_defaults_to_none() -> anyhow::Result<()> {
        // duration_ms 省略時は被弾側 Hit アニメ frame 0 duration にフォールバックさせるため None。
        let yaml = r"
shake_x: 1
count: 2
";
        let hs: HitStop = serde_saphyr::from_str(yaml)?;
        assert_eq!(hs.duration_ms, None);
        assert_eq!(hs.shake_x, 1);
        assert_eq!(hs.count, 2);
        Ok(())
    }

    #[test]
    fn attack_box_round_trip_with_meta() -> anyhow::Result<()> {
        let yaml = r"
hitbox:
  top_left: [10, 20]
  bottom_right: [30, 40]
  depth: 12
meta:
  damage: 40
  knockback_damage: 30
  knockback:
    vel_x: 120.0
    vel_y: 80.0
    vel_z: 0.0
";
        let ab: AttackBox = serde_saphyr::from_str(yaml)?;
        assert_eq!(ab.hitbox.top_left, [10, 20]);
        assert_eq!(ab.hitbox.bottom_right, [30, 40]);
        assert_eq!(ab.hitbox.depth, Some(12));
        let meta = ab.meta.expect("meta should be present");
        assert_eq!(meta.damage, 40);
        assert_eq!(meta.knockback_damage, 30);
        assert!((meta.knockback.vel_x - 120.0).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn attack_box_without_meta_deserializes() -> anyhow::Result<()> {
        let yaml = r"
hitbox:
  top_left: [0, 0]
  bottom_right: [10, 10]
";
        let ab: AttackBox = serde_saphyr::from_str(yaml)?;
        assert!(ab.meta.is_none());
        assert_eq!(ab.hitbox.depth, None);
        Ok(())
    }

    #[test]
    fn frame_attack_box_overrides_round_trip() -> anyhow::Result<()> {
        // 既存 editor が書く形 (hitbox / meta 両方あり) の YAML 互換性を維持する。
        let yaml = r"
index: 1
ticks: 10
attack_box_overrides:
- hitbox:
    top_left: [18, 28]
    bottom_right: [42, 48]
  meta:
    damage: 40
layers: []
";
        let frame: Frame = serde_saphyr::from_str(yaml)?;
        let overrides = frame
            .attack_box_overrides
            .as_ref()
            .expect("overrides should be present");
        assert_eq!(overrides.len(), 1);
        let hb = overrides[0]
            .hitbox
            .as_ref()
            .expect("hitbox should be present");
        assert_eq!(hb.top_left, [18, 28]);
        assert_eq!(
            overrides[0]
                .meta
                .as_ref()
                .expect("meta should be present")
                .damage,
            40
        );
        Ok(())
    }

    #[test]
    fn attack_box_override_hitbox_only_omits_meta() -> anyhow::Result<()> {
        // partial override: hitbox だけ書く (meta は sprite から継承される想定)。
        let yaml = r"
hitbox:
  top_left: [10, 20]
  bottom_right: [30, 40]
";
        let ov: AttackBoxOverride = serde_saphyr::from_str(yaml)?;
        assert!(ov.hitbox.is_some());
        assert!(ov.meta.is_none());
        Ok(())
    }

    #[test]
    fn attack_box_override_meta_only_omits_hitbox() -> anyhow::Result<()> {
        // partial override: meta だけ書く (hitbox は sprite から継承される想定)。
        let yaml = r"
meta:
  damage: 75
";
        let ov: AttackBoxOverride = serde_saphyr::from_str(yaml)?;
        assert!(ov.hitbox.is_none());
        assert_eq!(ov.meta.expect("meta should be present").damage, 75);
        Ok(())
    }

    #[test]
    fn attack_box_override_empty_object_is_noop() -> anyhow::Result<()> {
        // 両方 None: sprite を上書きしない (= sprite をそのまま使う)。
        let yaml = "{}";
        let ov: AttackBoxOverride = serde_saphyr::from_str(yaml)?;
        assert!(ov.hitbox.is_none());
        assert!(ov.meta.is_none());
        Ok(())
    }

    // === Sound / SoundGroup (ADR-0019) ===

    fn make_group(weights: &[f32]) -> SoundGroup {
        SoundGroup {
            name: "g".into(),
            number: 1,
            sounds: weights
                .iter()
                .enumerate()
                .map(|(i, &w)| Sound {
                    index: u32::try_from(i).unwrap_or(u32::MAX),
                    path: format!("{i}.wav"),
                    volume: 1.0,
                    weight: w,
                })
                .collect(),
        }
    }

    #[test]
    fn sound_group_pick_empty_returns_none() {
        let g = make_group(&[]);
        assert!(g.pick(|| 0.5).is_none());
    }

    #[test]
    fn sound_group_pick_single_returns_that_sound() {
        let g = make_group(&[1.0]);
        let s = g.pick(|| 0.5).expect("pick");
        assert_eq!(s.index, 0);
    }

    #[test]
    fn sound_group_pick_uniform_weights_distributes_by_roll() {
        // 3 要素 weight=1 (total=3)。roll=0.0 → idx 0, roll=0.5 → idx 1, roll=0.9 → idx 2。
        let g = make_group(&[1.0, 1.0, 1.0]);
        assert_eq!(g.pick(|| 0.0).expect("pick").index, 0);
        assert_eq!(g.pick(|| 0.5).expect("pick").index, 1);
        assert_eq!(g.pick(|| 0.9).expect("pick").index, 2);
    }

    #[test]
    fn sound_group_pick_zero_weight_falls_back_to_one() {
        // 全 weight=0 → effective_weight 経由で全部 1.0 として扱われ等確率。
        let g = make_group(&[0.0, 0.0, 0.0]);
        assert_eq!(g.pick(|| 0.0).expect("pick").index, 0);
        assert_eq!(g.pick(|| 0.5).expect("pick").index, 1);
        assert_eq!(g.pick(|| 0.9).expect("pick").index, 2);
    }

    #[test]
    fn sound_group_pick_negative_weight_falls_back_to_one() {
        // 負値 / NaN は effective_weight で 1.0 に倒れる。
        let g = make_group(&[-1.0, 1.0]);
        // total = 1.0 + 1.0 = 2.0 → roll=0.0 → idx 0
        assert_eq!(g.pick(|| 0.0).expect("pick").index, 0);
        assert_eq!(g.pick(|| 0.6).expect("pick").index, 1);
    }

    #[test]
    fn sound_group_pick_weighted_biases_toward_heavy() {
        // weight 比率 1 : 3 (total 4)。roll=0.0 → idx 0、roll=0.249 → idx 0、
        // roll=0.251 → idx 1。
        let g = make_group(&[1.0, 3.0]);
        assert_eq!(g.pick(|| 0.0).expect("pick").index, 0);
        assert_eq!(g.pick(|| 0.249).expect("pick").index, 0);
        assert_eq!(g.pick(|| 0.251).expect("pick").index, 1);
        assert_eq!(g.pick(|| 0.99).expect("pick").index, 1);
    }

    #[test]
    fn sound_group_pick_returns_last_on_float_overshoot() {
        // roll が total を僅かに超える浮動小数誤差ケース。`rand=1.0` でも末尾 fallback で必ず Some。
        let g = make_group(&[1.0, 1.0]);
        let s = g.pick(|| 1.0).expect("must fall back to last on overshoot");
        assert_eq!(s.index, 1);
    }

    #[test]
    fn sound_group_round_trip_from_yaml() -> anyhow::Result<()> {
        let yaml = r"
number: 1
sounds:
- index: 0
  path: pain_a.wav
  volume: 0.8
  weight: 2.0
- index: 1
  path: pain_b.wav
  volume: 0.8
";
        let g: SoundGroup = serde_saphyr::from_str(yaml)?;
        assert_eq!(g.number, 1);
        assert_eq!(g.sounds.len(), 2);
        assert_eq!(g.sounds[0].path, "pain_a.wav");
        assert!((g.sounds[0].weight - 2.0).abs() < f32::EPSILON);
        // weight 省略は 0.0 (= engine 側で 1.0 にフォールバック)
        assert!((g.sounds[1].weight).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn frame_sound_round_trip_from_yaml() -> anyhow::Result<()> {
        let yaml = r"
index: 0
ticks: 5
sound:
  number: 3
  delay_ms: 50
layers: []
";
        let f: Frame = serde_saphyr::from_str(yaml)?;
        let s = f.sound.expect("sound should be Some");
        assert_eq!(s.number, Some(3));
        assert_eq!(s.delay_ms, 50);
        assert!(s.on_hit.is_none());
        assert!(s.on_guard.is_none());
        Ok(())
    }

    #[test]
    fn frame_sound_hit_guard_round_trip_from_yaml() -> anyhow::Result<()> {
        // ADR-0034: on_hit / on_guard を含む新スキーマの round-trip。number 省略も許可。
        let yaml = r"
index: 0
ticks: 5
sound:
  on_hit: 7
  on_guard: 8
layers: []
";
        let f: Frame = serde_saphyr::from_str(yaml)?;
        let s = f.sound.expect("sound should be Some");
        assert!(s.number.is_none(), "number 省略時は None");
        assert_eq!(s.on_hit, Some(7));
        assert_eq!(s.on_guard, Some(8));
        Ok(())
    }

    #[test]
    fn frame_sound_all_three_fields_round_trip() -> anyhow::Result<()> {
        // 3 系統全部書く YAML も読める (Idle/Hit/Guarded のいずれでも何か鳴らす狙い)。
        let yaml = r"
index: 0
ticks: 5
sound:
  number: 1
  on_hit: 2
  on_guard: 3
  delay_ms: 30
layers: []
";
        let f: Frame = serde_saphyr::from_str(yaml)?;
        let s = f.sound.expect("sound should be Some");
        assert_eq!(s.number, Some(1));
        assert_eq!(s.on_hit, Some(2));
        assert_eq!(s.on_guard, Some(3));
        assert_eq!(s.delay_ms, 30);
        Ok(())
    }

    #[test]
    fn frame_without_sound_field_is_none() -> anyhow::Result<()> {
        let yaml = r"
index: 0
ticks: 5
layers: []
";
        let f: Frame = serde_saphyr::from_str(yaml)?;
        assert!(f.sound.is_none());
        Ok(())
    }

    #[test]
    fn character_find_sound_group_round_trip() {
        let mut c = Character::default();
        c.sound_groups.insert(7, make_group(&[1.0]));
        assert!(c.find_sound_group(7).is_some());
        assert!(c.find_sound_group(99).is_none());
    }

    // === AiConfig / MeleeConfig (ADR-0035) ===

    #[test]
    #[allow(clippy::float_cmp)]
    fn melee_config_default_matches_engine_constants() {
        let m = MeleeConfig::default();
        assert_eq!(
            m.engagement.chase_enter_range_px,
            DEFAULT_AI_CHASE_ENTER_RANGE_PX
        );
        assert_eq!(
            m.engagement.chase_exit_range_px,
            DEFAULT_AI_CHASE_EXIT_RANGE_PX
        );
        assert_eq!(
            m.engagement.attack_enter_range_px,
            DEFAULT_AI_ATTACK_ENTER_RANGE_PX
        );
        assert_eq!(
            m.engagement.attack_exit_range_px,
            DEFAULT_AI_ATTACK_EXIT_RANGE_PX
        );
        assert_eq!(
            m.engagement.attack_cooldown_ticks,
            DEFAULT_AI_ATTACK_COOLDOWN_TICKS
        );
        assert_eq!(
            m.engagement.decision_interval_ticks,
            DEFAULT_AI_DECISION_INTERVAL_TICKS
        );
        assert_eq!(m.engagement.min_dwell_ticks, DEFAULT_AI_MIN_DWELL_TICKS);
    }

    #[test]
    fn ai_config_melee_yaml_round_trip() -> anyhow::Result<()> {
        // ADR-0039: `EngagementConfig` の `#[serde(flatten)]` で flat YAML schema を保つ。
        let yaml = r"
kind: melee
chase_enter_range_px: 150.0
chase_exit_range_px: 180.0
attack_enter_range_px: 30.0
attack_exit_range_px: 40.0
attack_cooldown_ticks: 45
decision_interval_ticks: 3
min_dwell_ticks: 10
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Melee(m) = ai else {
            panic!("expected AiConfig::Melee");
        };
        assert!((m.engagement.chase_enter_range_px - 150.0).abs() < f32::EPSILON);
        assert!((m.engagement.attack_enter_range_px - 30.0).abs() < f32::EPSILON);
        assert_eq!(m.engagement.attack_cooldown_ticks, 45);
        assert_eq!(m.engagement.decision_interval_ticks, 3);
        Ok(())
    }

    #[test]
    fn ai_config_melee_partial_yaml_fills_defaults() -> anyhow::Result<()> {
        // 部分指定 (chase_enter_range_px のみ書く) で残りが DEFAULT_AI_* で補完されること。
        let yaml = r"
kind: melee
chase_enter_range_px: 150.0
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Melee(m) = ai else {
            panic!("expected AiConfig::Melee");
        };
        assert!((m.engagement.chase_enter_range_px - 150.0).abs() < f32::EPSILON);
        assert_eq!(
            m.engagement.attack_cooldown_ticks,
            DEFAULT_AI_ATTACK_COOLDOWN_TICKS
        );
        assert_eq!(m.engagement.min_dwell_ticks, DEFAULT_AI_MIN_DWELL_TICKS);
        Ok(())
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn ally_config_default_matches_engine_constants() {
        let a = AllyConfig::default();
        assert_eq!(a.follow_distance_min_px, DEFAULT_AI_FOLLOW_DISTANCE_MIN_PX);
        assert_eq!(a.follow_distance_max_px, DEFAULT_AI_FOLLOW_DISTANCE_MAX_PX);
        assert_eq!(
            a.engagement.chase_enter_range_px,
            DEFAULT_AI_CHASE_ENTER_RANGE_PX
        );
        assert_eq!(
            a.engagement.attack_enter_range_px,
            DEFAULT_AI_ATTACK_ENTER_RANGE_PX
        );
        assert_eq!(
            a.engagement.attack_cooldown_ticks,
            DEFAULT_AI_ATTACK_COOLDOWN_TICKS
        );
        assert_eq!(
            a.engagement.decision_interval_ticks,
            DEFAULT_AI_DECISION_INTERVAL_TICKS
        );
        assert_eq!(a.engagement.min_dwell_ticks, DEFAULT_AI_MIN_DWELL_TICKS);
    }

    #[test]
    fn ai_config_ally_yaml_round_trip() -> anyhow::Result<()> {
        let yaml = r"
kind: ally
follow_distance_min_px: 50.0
follow_distance_max_px: 100.0
chase_enter_range_px: 180.0
attack_cooldown_ticks: 30
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Ally(a) = ai else {
            panic!("expected AiConfig::Ally");
        };
        assert!((a.follow_distance_min_px - 50.0).abs() < f32::EPSILON);
        assert!((a.follow_distance_max_px - 100.0).abs() < f32::EPSILON);
        assert!((a.engagement.chase_enter_range_px - 180.0).abs() < f32::EPSILON);
        assert_eq!(a.engagement.attack_cooldown_ticks, 30);
        // 部分指定で残りが DEFAULT_AI_* で補完されること。
        assert_eq!(a.engagement.min_dwell_ticks, DEFAULT_AI_MIN_DWELL_TICKS);
        Ok(())
    }

    #[test]
    fn character_without_ai_field_yields_none() -> anyhow::Result<()> {
        // `ai:` 行が書かれていない既存 YAML (= hero 等) は `Character.ai` が None になる
        // (= AI Brain を attach しない、後方互換)。
        let yaml = r"
name: hero
hp: 100
";
        let c: Character = serde_saphyr::from_str(yaml)?;
        assert!(c.ai.is_none());
        Ok(())
    }

    #[test]
    fn character_with_ai_melee_field_parses() -> anyhow::Result<()> {
        let yaml = r"
name: grunt
ai:
  kind: melee
  attack_enter_range_px: 25.0
";
        let c: Character = serde_saphyr::from_str(yaml)?;
        let Some(AiConfig::Melee(m)) = c.ai else {
            panic!("expected AiConfig::Melee");
        };
        assert!((m.engagement.attack_enter_range_px - 25.0).abs() < f32::EPSILON);
        Ok(())
    }

    // === BotConfig / AiConfig::Bot (ADR-0038) ===

    #[test]
    #[allow(clippy::float_cmp)]
    fn bot_config_default_matches_engine_constants() {
        let b = BotConfig::default();
        assert_eq!(
            b.engagement.chase_enter_range_px,
            DEFAULT_AI_CHASE_ENTER_RANGE_PX
        );
        assert_eq!(
            b.engagement.attack_enter_range_px,
            DEFAULT_AI_ATTACK_ENTER_RANGE_PX
        );
        assert_eq!(
            b.engagement.attack_cooldown_ticks,
            DEFAULT_AI_ATTACK_COOLDOWN_TICKS
        );
        assert_eq!(
            b.engagement.decision_interval_ticks,
            DEFAULT_AI_DECISION_INTERVAL_TICKS
        );
        assert_eq!(b.engagement.min_dwell_ticks, DEFAULT_AI_MIN_DWELL_TICKS);
    }

    #[test]
    fn ai_config_bot_yaml_round_trip() -> anyhow::Result<()> {
        let yaml = r"
kind: bot
chase_enter_range_px: 150.0
chase_exit_range_px: 180.0
attack_enter_range_px: 30.0
attack_exit_range_px: 40.0
attack_cooldown_ticks: 45
decision_interval_ticks: 3
min_dwell_ticks: 10
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Bot(b) = ai else {
            panic!("expected AiConfig::Bot");
        };
        assert!((b.engagement.chase_enter_range_px - 150.0).abs() < f32::EPSILON);
        assert!((b.engagement.attack_enter_range_px - 30.0).abs() < f32::EPSILON);
        assert_eq!(b.engagement.attack_cooldown_ticks, 45);
        assert_eq!(b.engagement.decision_interval_ticks, 3);
        Ok(())
    }

    #[test]
    fn ai_config_bot_partial_yaml_fills_defaults() -> anyhow::Result<()> {
        let yaml = r"
kind: bot
chase_enter_range_px: 150.0
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Bot(b) = ai else {
            panic!("expected AiConfig::Bot");
        };
        assert!((b.engagement.chase_enter_range_px - 150.0).abs() < f32::EPSILON);
        assert_eq!(
            b.engagement.attack_cooldown_ticks,
            DEFAULT_AI_ATTACK_COOLDOWN_TICKS
        );
        assert_eq!(b.engagement.min_dwell_ticks, DEFAULT_AI_MIN_DWELL_TICKS);
        Ok(())
    }

    #[test]
    fn character_with_ai_bot_field_parses() -> anyhow::Result<()> {
        let yaml = r"
name: bot-hero
ai:
  kind: bot
  attack_enter_range_px: 30.0
";
        let c: Character = serde_saphyr::from_str(yaml)?;
        let Some(AiConfig::Bot(b)) = c.ai else {
            panic!("expected AiConfig::Bot");
        };
        assert!((b.engagement.attack_enter_range_px - 30.0).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn bot_config_into_melee_config_preserves_fields() {
        // ADR-0038/0039: BotConfig::into_melee_config は BotBrain attach 時に MeleeConfig へ
        // 詰め替えるための薄い変換。EngagementConfig 共有 + selector も持ち越されることを担保。
        let b = BotConfig {
            engagement: EngagementConfig {
                chase_enter_range_px: 1.0,
                chase_exit_range_px: 2.0,
                attack_enter_range_px: 3.0,
                attack_exit_range_px: 4.0,
                attack_cooldown_ticks: 5,
                decision_interval_ticks: 6,
                min_dwell_ticks: 7,
            },
            selector: TargetSelector::LastEngaged,
        };
        let m = b.into_melee_config();
        assert_eq!(m.engagement.chase_enter_range_px, 1.0);
        assert_eq!(m.engagement.chase_exit_range_px, 2.0);
        assert_eq!(m.engagement.attack_enter_range_px, 3.0);
        assert_eq!(m.engagement.attack_exit_range_px, 4.0);
        assert_eq!(m.engagement.attack_cooldown_ticks, 5);
        assert_eq!(m.engagement.decision_interval_ticks, 6);
        assert_eq!(m.engagement.min_dwell_ticks, 7);
        assert_eq!(m.selector, TargetSelector::LastEngaged);
    }

    // === TargetSelector (ADR-0039) ===

    #[test]
    fn target_selector_default_is_nearest() {
        // ADR-0039: 既存 YAML が selector 未指定でも Nearest にフォールバックする = 既存挙動互換。
        assert_eq!(TargetSelector::default(), TargetSelector::Nearest);
    }

    #[test]
    fn melee_config_default_selector_is_nearest() {
        // Config 経由でも default で Nearest になることを担保 (`#[serde(default)]` 経由)。
        assert_eq!(MeleeConfig::default().selector, TargetSelector::Nearest);
        assert_eq!(AllyConfig::default().selector, TargetSelector::Nearest);
        assert_eq!(BotConfig::default().selector, TargetSelector::Nearest);
    }

    #[test]
    fn ai_config_melee_yaml_without_selector_yields_nearest() -> anyhow::Result<()> {
        // ADR-0039: 旧 YAML (selector 行なし) は Nearest を埋める = 既存 sample-projects/minimal
        // の grunt YAML が無編集で読める。
        let yaml = r"
kind: melee
chase_enter_range_px: 150.0
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Melee(m) = ai else {
            panic!("expected AiConfig::Melee");
        };
        assert_eq!(m.selector, TargetSelector::Nearest);
        Ok(())
    }

    #[test]
    fn ai_config_melee_yaml_with_selector_last_engaged_round_trip() -> anyhow::Result<()> {
        let yaml = r"
kind: melee
chase_enter_range_px: 150.0
selector: last_engaged
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Melee(m) = ai else {
            panic!("expected AiConfig::Melee");
        };
        assert_eq!(m.selector, TargetSelector::LastEngaged);
        Ok(())
    }

    #[test]
    fn ai_config_ally_yaml_with_selector_last_engaged_round_trip() -> anyhow::Result<()> {
        // ADR-0039 Phase 2 補追動機 (Ally の継続追跡) の YAML 例。
        let yaml = r"
kind: ally
follow_distance_min_px: 40.0
selector: last_engaged
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Ally(a) = ai else {
            panic!("expected AiConfig::Ally");
        };
        assert_eq!(a.selector, TargetSelector::LastEngaged);
        Ok(())
    }

    #[test]
    fn ai_config_bot_yaml_with_selector_round_trip() -> anyhow::Result<()> {
        let yaml = r"
kind: bot
selector: nearest
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Bot(b) = ai else {
            panic!("expected AiConfig::Bot");
        };
        assert_eq!(b.selector, TargetSelector::Nearest);
        Ok(())
    }

    #[test]
    fn target_selector_stub_variants_round_trip() -> anyhow::Result<()> {
        // ADR-0039: Random / WeightedByThreat は variant のみ実装 (stub)。YAML を書いた時点で
        // deserialize できることだけ確認する (実行時挙動は ai.rs の warn fallback test 側)。
        let yaml = r"
kind: melee
selector: random
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Melee(m) = ai else {
            panic!("expected AiConfig::Melee");
        };
        assert_eq!(m.selector, TargetSelector::Random);

        let yaml = r"
kind: melee
selector: weighted_by_threat
";
        let ai: AiConfig = serde_saphyr::from_str(yaml)?;
        let AiConfig::Melee(m) = ai else {
            panic!("expected AiConfig::Melee");
        };
        assert_eq!(m.selector, TargetSelector::WeightedByThreat);
        Ok(())
    }
}
