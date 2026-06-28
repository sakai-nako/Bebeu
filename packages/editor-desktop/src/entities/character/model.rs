use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::shared::{AttackBox, AttackBoxMeta, AttackBoxOverride, FlipMode, HitBox};

/// Character.depth の既定値 (world Z 厚み)。HitBox.depth が None の box はこの値にフォールバックする。
/// 値の根拠: Stage 2 で導入した Level の Z bounds (-40..40 = 80) の 1/5 程度を 1 体ぶんの厚みとし、
/// 16 を起点にする。実プロジェクトでは Character ごとに調整する想定。
pub const DEFAULT_CHARACTER_DEPTH: u32 = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Character {
    pub name: String,
    pub thumbnail_path: String,
    pub hp: u32,
    /// world Z 軸の厚み (奥行き)。HitBox.depth が None の box が参照するベース値。
    /// 既定 = `DEFAULT_CHARACTER_DEPTH` (16)。YAML 省略時はこの値にフォールバックする。
    #[serde(default = "default_character_depth")]
    pub depth: u32,
    /// ADR-0031: HUD の `enemy_hp_bar` 等が `target: { tag: "boss" }` で参照する任意ラベル。
    /// engine 側 Character struct と対称。Player 側には影響しない。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// 物理パラメータ (重力 / ジャンプ初速 / Knockback ゲージ / バウンス / 摩擦 / 各種 timer)。
    /// YAML 省略時は `CharacterPhysics::default()`、各フィールドも個別に `#[serde(default)]` で
    /// 部分的省略を許す。詳細は `CharacterPhysics` を参照。
    #[serde(default)]
    pub physics: CharacterPhysics,
    // sprite_groups は Character の YAML には書き込まれず、Repository が
    // {character}/sprite-groups/*.yml を walk して populate する。
    #[serde(skip)]
    pub sprite_groups: Vec<SpriteGroup>,
    // animations も同様に {character}/animations/*.yml から populate される。
    #[serde(skip)]
    pub animations: Vec<Animation>,
    // sound_groups も同様に {character}/sound-groups/*.yml から populate される。
    #[serde(skip)]
    pub sound_groups: Vec<SoundGroup>,
}

fn default_character_depth() -> u32 {
    DEFAULT_CHARACTER_DEPTH
}

/// Character の物理パラメータ。
///
/// 1 キャラぶんの「重さ感」「跳ね方」「ダウン挙動」を決める。Level.gravity_scale との合成で
/// 演出差 (月面 / 水中) も作れる。各フィールドの単位と既定値は engine 側 `Physics` と対称。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterPhysics {
    /// 重力加速度 (px/s²)。+ で下向きに引かれる強さ。実効値は `Level.gravity_scale` を掛けたもの。
    pub gravity: f32,
    /// 自発ジャンプ時の初速 (px/s)。`movement.State.VelY` に充填される。
    pub jump_velocity_y: f32,
    /// Knockback ゲージの最大値 (耐久ポイント)。Hit を受けるたび `knockback_damage` で減算され、
    /// 0 以下で Knockback (吹っ飛び) 発動 → full 回復。
    pub knockback_threshold: u32,
    /// Knockback の被軽減率。0..1。実効 = 攻撃側の knockback * (1 - resistance)。
    pub knockback_resistance: f32,
    /// バウンス回数の上限。`KnockbackDown` で着地したとき残バウンス > 0 ならバウンス。
    pub bounce_count: u32,
    /// バウンス時の VelY 反転減衰率 (0..1)。0.5 で半分の高さまで跳ねる。
    pub bounce_dampening: f32,
    /// 地面スライド時の X/Z 摩擦 (px/s²)。`ActionSlide` のみ適用される。
    pub ground_friction: f32,
    /// Hit (地上小硬直) 後、Knockback ゲージが full 回復するまでの待ち時間 (ms)。
    pub hit_recovery_ms: u32,
    /// LieDown Animation が未登録 or is_loop=true のときの固定 timer (ms)。
    /// is_loop=false の Animation を登録した場合は Animation 長が優先される。
    pub lie_down_duration_ms: u32,
    /// Rise Animation が未登録 or is_loop=true のときの固定 timer (ms)。
    /// is_loop=false の Animation を登録した場合は Animation 長が優先される。
    pub rise_duration_ms: u32,
    /// 1 連続コンボあたりの空中再被弾 (ジャグル) 最大回数。`Combatant.juggle_count` がこれを
    /// 超えた airborne hit は **完全無敵** (damage / state / gauge / consumed 全て不発、
    /// AABB ヒットしても素通り) になる (= 永久パターン回避)。
    /// Rise → Idle で counter は reset される。
    pub max_juggle_count: u32,
    /// 1 連続コンボあたりの DownHit (倒れ中被弾) 最大回数。`Combatant.down_hit_count` がこれを
    /// 超えた down hit は **完全無敵** (damage / state / gauge / consumed 全て不発、AABB
    /// ヒットしても素通り) になる (= 倒れたまま無敵、永久パターン回避)。
    /// Rise → Idle で counter は reset される。
    pub max_down_hit_count: u32,
    /// Guard ゲージの初期値 / max (ADR-0028)。`AttackBoxMeta.guard_damage` で削られて
    /// 0 以下になると GuardBreak 発動。
    pub guard_break_threshold: u32,
    /// 最後にガード被弾してから何 ms で `guard_gauge` を full 回復するか (ADR-0028)。
    /// `hit_recovery_ms` と同型の自然回復モデル。
    pub guard_recovery_ms: u32,
    /// GuardBreak 発動時に被弾側へ充填する吹っ飛びベクトル (ADR-0028)。
    /// `KnockbackVec` と同じく `vel_x` は「攻撃側前方 = +」基準で書き、scene 側で Facing 反転する。
    pub guard_break_knockback: crate::shared::KnockbackVec,
}

impl Default for CharacterPhysics {
    fn default() -> Self {
        Self {
            gravity: 800.0,
            jump_velocity_y: 200.0,
            knockback_threshold: 100,
            knockback_resistance: 0.0,
            bounce_count: 1,
            bounce_dampening: 0.5,
            ground_friction: 600.0,
            hit_recovery_ms: 1500,
            lie_down_duration_ms: 800,
            rise_duration_ms: 300,
            max_juggle_count: 3,
            max_down_hit_count: 3,
            guard_break_threshold: 100,
            guard_recovery_ms: 1200,
            guard_break_knockback: crate::shared::KnockbackVec {
                vel_x: 100.0,
                vel_y: 150.0,
                vel_z: 0.0,
            },
        }
    }
}

impl Character {
    /// `Layer` の参照 (`sprite_group_number`, `sprite_index`) から `(SpriteGroup, Sprite)` を解決する。
    /// 見つからなければ None（描画側でプレースホルダ表示）。
    #[must_use]
    pub fn find_sprite(
        &self,
        sprite_group_number: u32,
        sprite_index: u32,
    ) -> Option<(&SpriteGroup, &Sprite)> {
        let group = self
            .sprite_groups
            .iter()
            .find(|g| g.number == sprite_group_number)?;
        let sprite = group.sprites.iter().find(|s| s.index == sprite_index)?;
        Some((group, sprite))
    }

    /// `Frame.sound` で参照される `SoundGroup` を `number` から解決する。
    /// engine 側 `Character::FindSoundGroup` と対称。見つからなければ None。
    #[must_use]
    pub fn find_sound_group(&self, number: u32) -> Option<&SoundGroup> {
        self.sound_groups.iter().find(|g| g.number == number)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpriteGroup {
    pub name: String,
    pub number: u32,
    pub sprites: Vec<Sprite>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sprite {
    pub index: u32,
    pub path: String,
    pub pivot_point: [i32; 2],
    pub body_boxes: Option<Vec<HitBox>>,
    /// 攻撃判定。`AttackBox = HitBox + Option<AttackBoxMeta>` で、meta は攻撃のダメージ /
    /// Knockback ゲージ減算 / 吹っ飛びベクトルを保持する (`shared::AttackBox`)。
    /// 旧形式 (`Vec<HitBox>` を直接) は AttackBox 側の `serde(from = "RawAttackBox")` で
    /// deserialize 時に meta=None として吸収される。
    pub attack_boxes: Option<Vec<AttackBox>>,
    /// PNG header から読み取った image dimensions (width, height in pixels)。
    /// YAML には書かないので `#[serde(skip)]` で disk から読み込み時は `None`、
    /// `FilesystemRepository::get` 等の loader が PNG header を読んで埋める。
    /// SpriteCanvas / AnimationCanvas は zoom 倍 CSS px の explicit sizing にこれを使う
    /// (4K + 150% スケール対策。詳細は ui/README.md)。
    #[serde(skip)]
    pub dimensions: Option<[u32; 2]>,
}

/// Sprite に属する HitBox の参照。Body と Attack の 2 系列を index で指す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedBox {
    Body(usize),
    Attack(usize),
}

impl SelectedBox {
    /// 系列 (Body / Attack) のみを取り出す。
    #[must_use]
    pub fn kind(self) -> BoxKind {
        match self {
            Self::Body(_) => BoxKind::Body,
            Self::Attack(_) => BoxKind::Attack,
        }
    }

    /// box index を取り出す。
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Body(i) | Self::Attack(i) => i,
        }
    }
}

/// Body / Attack のみを区別する軽量 enum。`SelectedBox` から index を落とした表現で、
/// UI の色分け・ラベル付け、Frame override slot の選択など index に依存しない処理で使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxKind {
    Body,
    Attack,
}

impl BoxKind {
    /// `SelectedBox::Body(i)` / `SelectedBox::Attack(i)` を組み立てる。
    #[must_use]
    pub fn select(self, index: usize) -> SelectedBox {
        match self {
            Self::Body => SelectedBox::Body(index),
            Self::Attack => SelectedBox::Attack(index),
        }
    }

    /// 一覧見出し ("Body Boxes" / "Attack Boxes")。
    #[must_use]
    pub fn list_heading(self) -> &'static str {
        match self {
            Self::Body => "Body Boxes",
            Self::Attack => "Attack Boxes",
        }
    }

    /// 単体ラベル ("Body Box" / "Attack Box")。
    #[must_use]
    pub fn singular_label(self) -> &'static str {
        match self {
            Self::Body => "Body Box",
            Self::Attack => "Attack Box",
        }
    }

    /// バッジ・索引表示の prefix ("B" / "A")。`format!("{}{i}", kind.label_prefix())` で使う。
    #[must_use]
    pub fn label_prefix(self) -> &'static str {
        match self {
            Self::Body => "B",
            Self::Attack => "A",
        }
    }

    /// 識別子 ("body" / "attack")。radio button の name 属性等で使う。
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Attack => "attack",
        }
    }

    /// Inherit (read-only) 表示の border / 塗り（dashed の薄色）。
    /// Tailwind JIT スキャナーが完全なクラス名を必要とするので format! で組まずに返す。
    #[must_use]
    pub fn inherit_box_classes(self) -> &'static str {
        match self {
            Self::Body => "border-info/70 bg-info/10",
            Self::Attack => "border-error/70 bg-error/10",
        }
    }

    /// Override (interactive) 表示の border / 塗り。
    #[must_use]
    pub fn override_box_classes(self) -> &'static str {
        match self {
            Self::Body => "border-info bg-info/20",
            Self::Attack => "border-error bg-error/20",
        }
    }

    /// index バッジ (`B0` / `A0`) の塗り + 文字色。
    #[must_use]
    pub fn badge_classes(self) -> &'static str {
        match self {
            Self::Body => "bg-info text-info-content",
            Self::Attack => "bg-error text-error-content",
        }
    }

    /// 一覧 row 用の daisyUI badge クラス（`badge badge-{color} badge-sm`）。
    #[must_use]
    pub fn list_badge_classes(self) -> &'static str {
        match self {
            Self::Body => "badge badge-info badge-sm",
            Self::Attack => "badge badge-error badge-sm",
        }
    }

    /// Sprite 上の HitBox スライス相当を読み取る。Attack 側は AttackBox から hitbox 部分を
    /// 抜き出して `Vec<HitBox>` を新規に組むため `Cow::Owned` で返る (描画/iter 用途で
    /// 元の `&[HitBox]` API と互換)。
    #[must_use]
    pub fn sprite_hitbox_slice(self, sprite: &Sprite) -> Option<Cow<'_, [HitBox]>> {
        match self {
            Self::Body => sprite.body_boxes.as_deref().map(Cow::Borrowed),
            Self::Attack => sprite.attack_boxes.as_deref().map(|v| {
                let owned: Vec<HitBox> = v.iter().map(|ab| ab.hitbox.clone()).collect();
                Cow::Owned(owned)
            }),
        }
    }

    /// Sprite/Frame に格納されている box 数を返す (HitBox / AttackBox どちらでも)。
    /// Body と Attack 共通の高レベル API として、box の個数を Vec の型を知らずに取れる。
    /// Sprite の場合は `Vec::len`、`None` の場合は 0 を返す。
    #[must_use]
    pub fn sprite_box_count(self, sprite: &Sprite) -> usize {
        match self {
            Self::Body => sprite.body_boxes.as_deref().map_or(0, <[HitBox]>::len),
            Self::Attack => sprite.attack_boxes.as_deref().map_or(0, <[AttackBox]>::len),
        }
    }

    /// Frame override の HitBox スライス相当を読み取る。`sprite_hitbox_slice` と対称。
    /// 3 状態 (None=Inherit / Some(empty)=Disable / Some(non-empty)=Override) を保つ。
    /// Attack 側で `AttackBoxOverride.hitbox` が `None` (= sprite から継承中) の要素は
    /// `HitBox::new(0,0,0,0)` の placeholder で埋める (描画は呼び出し側で個別に skip)。
    /// 「override 配列の長さ」を保つために要素数は元と一致させる。
    #[must_use]
    pub fn frame_override_hitbox_slice(self, frame: &Frame) -> Option<Cow<'_, [HitBox]>> {
        match self {
            Self::Body => frame.body_box_overrides.as_deref().map(Cow::Borrowed),
            Self::Attack => frame.attack_box_overrides.as_deref().map(|v| {
                let owned: Vec<HitBox> = v
                    .iter()
                    .map(|ov| ov.hitbox.clone().unwrap_or_else(|| HitBox::new(0, 0, 0, 0)))
                    .collect();
                Cow::Owned(owned)
            }),
        }
    }

    /// Frame override の box 個数を返す。`Some` 含む 3 状態を保つ場合は
    /// `override_mode_count(...)` 経由で `Some(usize)` / `None` を直接見る。
    /// この関数は `None` を 0 にまるめる軽量カウント用。
    #[must_use]
    pub fn frame_override_box_count(self, frame: &Frame) -> usize {
        match self {
            Self::Body => frame
                .body_box_overrides
                .as_deref()
                .map_or(0, <[HitBox]>::len),
            Self::Attack => frame
                .attack_box_overrides
                .as_deref()
                .map_or(0, <[AttackBoxOverride]>::len),
        }
    }

    /// Frame override の状態を `Option<Vec の長さ>` で返す。`None` = Inherit、
    /// `Some(0)` = Disable、`Some(n>0)` = Override。overrides.rs の `BoxOverrideMode`
    /// から参照される単一情報源。
    #[must_use]
    pub fn frame_override_state(self, frame: &Frame) -> Option<usize> {
        match self {
            Self::Body => frame.body_box_overrides.as_ref().map(Vec::len),
            Self::Attack => frame.attack_box_overrides.as_ref().map(Vec::len),
        }
    }

    /// Attack 専用: 指定 index の override 要素で hitbox が `None` (= sprite から継承中)
    /// かどうか。Body は常に `false` (継承機構が無い)。canvas の dashed 描画判定や
    /// property panel の「(継承中)」表示判定に使う。
    #[must_use]
    pub fn is_frame_override_hitbox_inherited(self, frame: &Frame, index: usize) -> bool {
        match self {
            Self::Body => false,
            Self::Attack => frame
                .attack_box_overrides
                .as_deref()
                .and_then(|v| v.get(index))
                .is_some_and(|ov| ov.hitbox.is_none()),
        }
    }

    /// Attack 専用: 指定 index の override 要素で meta が `None` (= sprite から継承中)
    /// かどうか。Body は常に `false` (meta が存在しない)。
    #[must_use]
    pub fn is_frame_override_meta_inherited(self, frame: &Frame, index: usize) -> bool {
        match self {
            Self::Body => false,
            Self::Attack => frame
                .attack_box_overrides
                .as_deref()
                .and_then(|v| v.get(index))
                .is_some_and(|ov| ov.meta.is_none()),
        }
    }

    /// Frame override を Inherit に戻す (slot を `None` に)。
    pub fn set_frame_override_inherit(self, frame: &mut Frame) {
        match self {
            Self::Body => frame.body_box_overrides = None,
            Self::Attack => frame.attack_box_overrides = None,
        }
    }

    /// Frame override を Disable に切り替え (slot を `Some(empty)` に)。
    pub fn set_frame_override_disable(self, frame: &mut Frame) {
        match self {
            Self::Body => frame.body_box_overrides = Some(Vec::new()),
            Self::Attack => frame.attack_box_overrides = Some(Vec::new()),
        }
    }

    /// Frame override を Override に切り替え。既存 vec が空または None なら
    /// 1 要素入れて Override の意味を保つ。
    /// Attack 側は `AttackBoxOverride::inherit_hitbox_with_default_meta()`
    /// (hitbox=None, meta=Some(default)) で初期化する。これにより「box 自体は sprite を継承し、
    /// meta だけ上書き」が UI から最短手数で作れる。sprite に対応 index の box が無い
    /// 場合は呼び出し側で `replace_override_box` で hitbox を Some に書き換える。
    pub fn ensure_frame_override_present(self, frame: &mut Frame, default_hitbox: HitBox) {
        match self {
            Self::Body => {
                let slot = &mut frame.body_box_overrides;
                if slot.as_ref().is_none_or(Vec::is_empty) {
                    *slot = Some(vec![default_hitbox]);
                }
            }
            Self::Attack => {
                let slot = &mut frame.attack_box_overrides;
                if slot.as_ref().is_none_or(Vec::is_empty) {
                    *slot = Some(vec![AttackBoxOverride::inherit_hitbox_with_default_meta()]);
                }
            }
        }
    }

    /// Frame override の末尾に box を 1 つ追加する。Vec が None なら新規生成する。
    /// Attack 側は `AttackBoxOverride::inherit_hitbox_with_default_meta()`
    /// (hitbox=None, meta=Some(default)) で push する。
    /// 「box 自体は sprite を継承 + meta だけ上書き」を UI からの最短手数にする方針
    /// ([Add Box] 直後に damage 入力可能、hitbox 矩形は sprite を継承)。
    pub fn push_frame_override_box(self, frame: &mut Frame, default_hitbox: HitBox) {
        match self {
            Self::Body => {
                let v = frame.body_box_overrides.get_or_insert_with(Vec::new);
                v.push(default_hitbox);
            }
            Self::Attack => {
                let v = frame.attack_box_overrides.get_or_insert_with(Vec::new);
                let _ = default_hitbox; // hitbox は inherit を初期値とする (sprite から継承)
                v.push(AttackBoxOverride::inherit_hitbox_with_default_meta());
            }
        }
    }

    /// Frame override の box を 1 つ削除する。index が範囲外なら no-op。
    /// Vec が空になっても `None` には戻さない (Disable と区別するため、3 状態は
    /// overrides.rs 側の明示的なモード切替で管理する)。
    pub fn remove_frame_override_box(self, frame: &mut Frame, index: usize) {
        match self {
            Self::Body => {
                if let Some(v) = frame.body_box_overrides.as_mut()
                    && index < v.len()
                {
                    v.remove(index);
                }
            }
            Self::Attack => {
                if let Some(v) = frame.attack_box_overrides.as_mut()
                    && index < v.len()
                {
                    v.remove(index);
                }
            }
        }
    }

    /// Frame override の指定 index の HitBox を取得する。Attack の場合は
    /// `AttackBoxOverride.hitbox` (None = sprite から継承中) を返す。Body は常に `Some` (Body
    /// 側に継承機構が無いため slot が存在すれば必ず HitBox を持つ)。
    #[must_use]
    pub fn get_frame_override_hitbox(self, frame: &Frame, index: usize) -> Option<HitBox> {
        match self {
            Self::Body => frame
                .body_box_overrides
                .as_deref()
                .and_then(|v| v.get(index))
                .cloned(),
            Self::Attack => frame
                .attack_box_overrides
                .as_deref()
                .and_then(|v| v.get(index))
                .and_then(|ov| ov.hitbox.clone()),
        }
    }

    /// Frame override の指定 index の HitBox を差し替える。Attack の場合は
    /// `AttackBoxOverride.hitbox = Some(new_hitbox)` で **自動的に override 化** (= inherit
    /// 状態だった場合は明示 override に切り替わる)、`meta` は保持される。範囲外なら no-op。
    pub fn replace_frame_override_hitbox(
        self,
        frame: &mut Frame,
        index: usize,
        new_hitbox: HitBox,
    ) {
        match self {
            Self::Body => {
                if let Some(v) = frame.body_box_overrides.as_mut()
                    && let Some(slot) = v.get_mut(index)
                {
                    *slot = new_hitbox;
                }
            }
            Self::Attack => {
                if let Some(v) = frame.attack_box_overrides.as_mut()
                    && let Some(slot) = v.get_mut(index)
                {
                    slot.hitbox = Some(new_hitbox);
                }
            }
        }
    }

    /// Attack 専用: 指定 index の override 要素の `hitbox` を `None` に戻す (= sprite から
    /// 継承するモードに切り替える)。範囲外 / Body の場合は no-op。
    pub fn set_frame_override_hitbox_inherit(self, frame: &mut Frame, index: usize) {
        if let Self::Attack = self
            && let Some(v) = frame.attack_box_overrides.as_mut()
            && let Some(slot) = v.get_mut(index)
        {
            slot.hitbox = None;
        }
    }

    /// Attack 専用: 指定 index の override 要素の `meta` を `None` に戻す (= sprite から
    /// 継承するモードに切り替える)。範囲外 / Body の場合は no-op。
    pub fn set_frame_override_meta_inherit(self, frame: &mut Frame, index: usize) {
        if let Self::Attack = self
            && let Some(v) = frame.attack_box_overrides.as_mut()
            && let Some(slot) = v.get_mut(index)
        {
            slot.meta = None;
        }
    }
}

impl Sprite {
    /// 指定された HitBox を取得する。Attack の場合は `AttackBox.hitbox` を返す。
    /// 存在しなければ None。
    #[must_use]
    pub fn get_box(&self, target: SelectedBox) -> Option<HitBox> {
        match target {
            SelectedBox::Body(i) => self.body_boxes.as_deref().and_then(|v| v.get(i)).cloned(),
            SelectedBox::Attack(i) => self
                .attack_boxes
                .as_deref()
                .and_then(|v| v.get(i))
                .map(|ab| ab.hitbox.clone()),
        }
    }

    /// 指定インデックスの HitBox を置き換える。範囲外なら何もしない。
    /// Attack の場合は AttackBox.hitbox 部分のみ差し替え、meta は保持される。
    pub fn replace_box(&mut self, target: SelectedBox, new_box: HitBox) {
        match target {
            SelectedBox::Body(i) => {
                if let Some(boxes) = self.body_boxes.as_mut()
                    && let Some(slot) = boxes.get_mut(i)
                {
                    *slot = new_box;
                }
            }
            SelectedBox::Attack(i) => {
                if let Some(boxes) = self.attack_boxes.as_mut()
                    && let Some(slot) = boxes.get_mut(i)
                {
                    slot.hitbox = new_box;
                }
            }
        }
    }

    /// 指定された種別の Vec の末尾に HitBox を追加し、追加位置の index を返す。
    /// Vec が None なら新規生成する。Attack の場合は `AttackBox::from_hitbox(new_box)`
    /// (meta=None) で push する。
    pub fn push_box(&mut self, target: SelectedBox, new_box: HitBox) -> usize {
        match target {
            SelectedBox::Body(_) => {
                let boxes = self.body_boxes.get_or_insert_with(Vec::new);
                boxes.push(new_box);
                boxes.len() - 1
            }
            SelectedBox::Attack(_) => {
                let boxes = self.attack_boxes.get_or_insert_with(Vec::new);
                boxes.push(AttackBox::from_hitbox(new_box));
                boxes.len() - 1
            }
        }
    }

    /// 指定インデックスの box を削除する。削除後 Vec が空なら None に戻す（yml 規約と整合）。
    pub fn remove_box(&mut self, target: SelectedBox) {
        match target {
            SelectedBox::Body(i) => remove_at_or_clear(&mut self.body_boxes, i),
            SelectedBox::Attack(i) => remove_at_or_clear(&mut self.attack_boxes, i),
        }
    }

    /// Attack 専用: 指定 index の AttackBox 全体 (meta 含む) を取得する。
    #[must_use]
    pub fn get_attack_box(&self, index: usize) -> Option<&AttackBox> {
        self.attack_boxes.as_deref().and_then(|v| v.get(index))
    }

    /// Attack 専用: 指定 index の AttackBox の meta を差し替える。範囲外なら no-op。
    pub fn replace_attack_meta(&mut self, index: usize, new_meta: Option<AttackBoxMeta>) {
        if let Some(boxes) = self.attack_boxes.as_mut()
            && let Some(slot) = boxes.get_mut(index)
        {
            slot.meta = new_meta;
        }
    }
}

/// Vec から index を 1 つ remove する。範囲外なら no-op、空になったら slot 自体を None に戻す。
/// Sprite::remove_box の Body/Attack 共通ロジックを単一化するための内部ヘルパ。
fn remove_at_or_clear<T>(slot: &mut Option<Vec<T>>, index: usize) {
    if let Some(boxes) = slot.as_mut() {
        if index < boxes.len() {
            boxes.remove(index);
        }
        if boxes.is_empty() {
            *slot = None;
        }
    }
}

/// 同じ用途の音 (例: pain / death / 攻撃ボイス) をまとめた集合。`number` は yaml に書く
/// 識別子で、Frame.sound から参照される。複数 Sound を持てば SoundGroup.Pick (engine 側) で
/// `weight` に応じてランダムに 1 つ選ばれる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundGroup {
    pub name: String,
    pub number: u32,
    pub sounds: Vec<Sound>,
}

/// 1 つの音源ファイルとボリューム / 抽選重み。`weight` は省略 (Default = 0.0) なら engine 側で
/// 1.0 にフォールバックされる (= 全 Sound 等確率)。明示的に重みを書いた時だけ偏らせる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sound {
    pub index: u32,
    pub path: String,
    pub volume: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub weight: f32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    pub name: String,
    /// engine 側 State (Idle/Walk/Attack/Hit/Dead/Jump/Block) との semantic な紐付け。
    /// 各エンジン (内蔵 / ikemen / 将来 OpenBOR) は role + variant を自分の番号体系に写像する。
    /// 未設定の yaml は `Role::Custom` (= 役割なし) として読まれる。
    #[serde(default)]
    pub role: super::role::Role,
    /// Multi-cardinality role (Attack/Hit/Dead/Jump) の役割内 slot 番号 (0-indexed)。
    /// Single-cardinality role (Idle/Walk/Block/Custom) では使われず 0 固定。
    #[serde(default)]
    pub variant: u32,
    /// Custom role 専用の ikemen export 用 Action 番号。
    /// CNS state controller (`ChangeAnim, value = N`) から参照される独自番号として `.air` に出力される。
    /// 標準 role (Idle/Walk/...) は role+variant から Mugen 標準番号に写像されるので不要。
    /// `None` で「export しない」を明示。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_number: Option<u32>,
    pub is_loop: bool,
    pub loop_start_index: u32,
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub index: u32,
    /// 60Hz vsync tick (= 1/60 秒) 単位の pose 寿命。engine の `Frame.ticks` と
    /// 完全に同じ意味。例: `7` で 7 tick = 約 116.67 ms。yaml の field 名も `ticks`。
    pub ticks: u32,
    pub flip: Option<FlipMode>,
    pub pivot_point_offset: Option<[i32; 2]>,
    pub body_box_overrides: Option<Vec<HitBox>>,
    /// 攻撃判定の frame-override。各要素は `AttackBoxOverride { hitbox: Option<HitBox>,
    /// meta: Option<AttackBoxMeta> }` で、`hitbox` / `meta` を個別に Option 化しているので
    /// **field 単位の partial override** が可能 (例: hitbox は sprite から継承して meta だけ
    /// 上書き)。3 状態は Vec 全体で表す: `None`=Inherit (全 box が sprite を継承) /
    /// `Some(empty)`=Disable / `Some(non-empty)`=Override (個別 box は内部 field で partial 化)。
    /// 旧形式 (`Vec<HitBox>`) は `AttackBoxOverride` の serde 互換で吸収される。
    pub attack_box_overrides: Option<Vec<AttackBoxOverride>>,
    /// この frame に進入した瞬間に再生する Sound 参照。`None` で無音。
    /// engine 側で `Character.FindSoundGroup(number)` から SoundGroup を引き、`Pick` で
    /// `weight` 付きランダムに 1 つの Sound を選んで再生する。`delay_ms` の分だけ
    /// 再生開始を遅らせる。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<FrameSound>,
    pub layers: Vec<Layer>,
}

/// Frame に紐づく Sound 参照 + 再生遅延 (ADR-0019 / ADR-0034)。
///
/// 3 系統で attacker 側 attack 結果ごとに出し分ける:
/// - `number`: 既定 (= 振り音 / Hit voice / 通常時セリフ等、無条件で frame 進入時に latch
///   したい音全般)。`AttackOutcome::Idle` 時、または on_hit/on_guard が None のときの
///   フォールバック先
/// - `on_hit`: AttackBox が Hit したときに優先
/// - `on_guard`: AttackBox が Guard されたときに優先
///
/// engine 側 (`packages/engine/src/entities/character/model.rs`) と YAML 上互換。
/// `delay_ms` は 3 系統共通で frame 進入から再生開始までの遅延 (ms)。engine の dispatch
/// tick (≈16.667ms) 単位で丸まる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FrameSound {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_hit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_guard: Option<u32>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub delay_ms: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl Frame {
    /// 指定 index の override HitBox を取得。Attack の場合は `AttackBoxOverride.hitbox`
    /// (None = sprite から継承中) を返す。`Sprite::get_box` と対称。
    #[must_use]
    pub fn get_override_box(&self, target: SelectedBox) -> Option<HitBox> {
        match target {
            SelectedBox::Body(i) => self
                .body_box_overrides
                .as_deref()
                .and_then(|v| v.get(i))
                .cloned(),
            SelectedBox::Attack(i) => self
                .attack_box_overrides
                .as_deref()
                .and_then(|v| v.get(i))
                .and_then(|ov| ov.hitbox.clone()),
        }
    }

    /// 指定 index の override HitBox を置き換える。範囲外なら no-op。canvas 上の
    /// drag/resize 適用時に使う。Attack の場合は `AttackBoxOverride.hitbox =
    /// Some(new_box)` で **自動 override 化** (inherit 状態だった場合は明示 override に
    /// 切り替わる)、`meta` は保持される。
    pub fn replace_override_box(&mut self, target: SelectedBox, new_box: HitBox) {
        match target {
            SelectedBox::Body(i) => {
                if let Some(boxes) = self.body_box_overrides.as_mut()
                    && let Some(slot) = boxes.get_mut(i)
                {
                    *slot = new_box;
                }
            }
            SelectedBox::Attack(i) => {
                if let Some(boxes) = self.attack_box_overrides.as_mut()
                    && let Some(slot) = boxes.get_mut(i)
                {
                    slot.hitbox = Some(new_box);
                }
            }
        }
    }

    /// Attack 専用: 指定 index の AttackBoxOverride (hitbox / meta 各 Option) を取得する。
    /// `hitbox` / `meta` のいずれも `None` なら sprite から継承中。
    #[must_use]
    pub fn get_attack_override(&self, index: usize) -> Option<&AttackBoxOverride> {
        self.attack_box_overrides
            .as_deref()
            .and_then(|v| v.get(index))
    }

    /// Attack 専用: 指定 index の AttackBoxOverride の meta を差し替える (Some/None どちらも
    /// 取れる: None で sprite から継承)。範囲外なら no-op。
    pub fn replace_attack_override_meta(&mut self, index: usize, new_meta: Option<AttackBoxMeta>) {
        if let Some(boxes) = self.attack_box_overrides.as_mut()
            && let Some(slot) = boxes.get_mut(index)
        {
            slot.meta = new_meta;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub index: u32,
    /// 参照する SpriteGroup を **number** で指定する（name のリネーム耐性）。
    pub sprite_group_number: u32,
    /// SpriteGroup 内の Sprite を **index** で指定する（filename 変更耐性）。
    pub sprite_index: u32,
    /// 0.0 〜 1.0 の透明度。1.0 で完全不透明。
    pub transparency: f32,
    pub flip: Option<FlipMode>,
    pub pivot_point_offset: Option<[i32; 2]>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_with_overrides(
        body: Option<Vec<HitBox>>,
        attack: Option<Vec<AttackBoxOverride>>,
    ) -> Frame {
        Frame {
            index: 0,
            ticks: 0,
            flip: None,
            pivot_point_offset: None,
            body_box_overrides: body,
            attack_box_overrides: attack,
            sound: None,
            layers: Vec::new(),
        }
    }

    #[test]
    fn get_override_box_returns_none_for_missing_slot() {
        let f = frame_with_overrides(None, None);
        assert!(f.get_override_box(SelectedBox::Body(0)).is_none());
        assert!(f.get_override_box(SelectedBox::Attack(0)).is_none());
    }

    #[test]
    fn get_override_box_returns_none_for_out_of_range_index() {
        let f = frame_with_overrides(Some(vec![HitBox::new(0, 0, 1, 1)]), None);
        assert!(f.get_override_box(SelectedBox::Body(0)).is_some());
        assert!(f.get_override_box(SelectedBox::Body(1)).is_none());
    }

    #[test]
    fn replace_override_box_updates_target_slot() {
        let mut f = frame_with_overrides(Some(vec![HitBox::new(0, 0, 1, 1)]), None);
        f.replace_override_box(SelectedBox::Body(0), HitBox::new(2, 2, 4, 4));
        let got = f
            .get_override_box(SelectedBox::Body(0))
            .expect("box exists");
        assert_eq!(got.top_left(), [2, 2]);
        assert_eq!(got.bottom_right(), [4, 4]);
    }

    #[test]
    fn replace_override_box_is_noop_for_missing_slot() {
        let mut f = frame_with_overrides(None, None);
        f.replace_override_box(SelectedBox::Body(0), HitBox::new(2, 2, 4, 4));
        assert!(f.body_box_overrides.is_none());
    }

    #[test]
    fn replace_override_box_is_noop_for_out_of_range_index() {
        let mut f = frame_with_overrides(Some(vec![HitBox::new(0, 0, 1, 1)]), None);
        f.replace_override_box(SelectedBox::Body(5), HitBox::new(9, 9, 9, 9));
        let got = f
            .get_override_box(SelectedBox::Body(0))
            .expect("box exists");
        assert_eq!(got.top_left(), [0, 0]);
    }

    #[test]
    fn replace_override_box_preserves_attack_meta() {
        // Attack 側の hitbox 差し替えで meta が保持されることを確認する。
        let meta = AttackBoxMeta {
            damage: 10,
            knockback_damage: 20,
            ..Default::default()
        };
        let ov = AttackBoxOverride {
            hitbox: Some(HitBox::new(0, 0, 1, 1)),
            meta: Some(meta),
        };
        let mut f = frame_with_overrides(None, Some(vec![ov]));
        f.replace_override_box(SelectedBox::Attack(0), HitBox::new(5, 5, 10, 10));
        let updated = f.get_attack_override(0).expect("attack box exists");
        assert_eq!(
            updated.hitbox.as_ref().expect("hitbox set").top_left(),
            [5, 5]
        );
        assert_eq!(updated.meta, Some(meta));
    }

    #[test]
    fn replace_attack_override_meta_updates_meta_only() {
        let ov = AttackBoxOverride {
            hitbox: Some(HitBox::new(0, 0, 1, 1)),
            meta: None,
        };
        let mut f = frame_with_overrides(None, Some(vec![ov]));
        let new_meta = AttackBoxMeta {
            damage: 30,
            ..Default::default()
        };
        f.replace_attack_override_meta(0, Some(new_meta));
        let updated = f.get_attack_override(0).expect("attack box exists");
        assert_eq!(updated.meta, Some(new_meta));
        // hitbox は変わらない
        let hb = updated.hitbox.as_ref().expect("hitbox set");
        assert_eq!(hb.top_left(), [0, 0]);
        assert_eq!(hb.bottom_right(), [1, 1]);
    }

    #[test]
    fn replace_override_box_promotes_inherit_to_override_on_attack() {
        // Attack の hitbox=None (inherit 状態) に drag/resize 適用すると Some に昇格する。
        let ov = AttackBoxOverride {
            hitbox: None,
            meta: Some(AttackBoxMeta::default()),
        };
        let mut f = frame_with_overrides(None, Some(vec![ov]));
        f.replace_override_box(SelectedBox::Attack(0), HitBox::new(2, 3, 10, 12));
        let updated = f.get_attack_override(0).expect("attack box exists");
        let hb = updated.hitbox.as_ref().expect("hitbox promoted to Some");
        assert_eq!(hb.top_left(), [2, 3]);
        assert_eq!(hb.bottom_right(), [10, 12]);
        // meta は保持
        assert_eq!(updated.meta, Some(AttackBoxMeta::default()));
    }

    #[test]
    fn box_kind_is_frame_override_hitbox_inherited_returns_true_when_hitbox_none() {
        let ov = AttackBoxOverride {
            hitbox: None,
            meta: Some(AttackBoxMeta::default()),
        };
        let f = frame_with_overrides(None, Some(vec![ov]));
        assert!(BoxKind::Attack.is_frame_override_hitbox_inherited(&f, 0));
        assert!(!BoxKind::Attack.is_frame_override_meta_inherited(&f, 0));
    }

    #[test]
    fn box_kind_is_frame_override_meta_inherited_returns_true_when_meta_none() {
        let ov = AttackBoxOverride {
            hitbox: Some(HitBox::new(0, 0, 1, 1)),
            meta: None,
        };
        let f = frame_with_overrides(None, Some(vec![ov]));
        assert!(!BoxKind::Attack.is_frame_override_hitbox_inherited(&f, 0));
        assert!(BoxKind::Attack.is_frame_override_meta_inherited(&f, 0));
    }

    #[test]
    fn box_kind_set_frame_override_hitbox_inherit_clears_hitbox() {
        let ov = AttackBoxOverride::full(HitBox::new(0, 0, 1, 1), AttackBoxMeta::default());
        let mut f = frame_with_overrides(None, Some(vec![ov]));
        BoxKind::Attack.set_frame_override_hitbox_inherit(&mut f, 0);
        let updated = f.get_attack_override(0).expect("attack box exists");
        assert!(updated.hitbox.is_none());
        assert!(updated.meta.is_some());
    }

    #[test]
    fn box_kind_set_frame_override_meta_inherit_clears_meta() {
        let ov = AttackBoxOverride::full(HitBox::new(0, 0, 1, 1), AttackBoxMeta::default());
        let mut f = frame_with_overrides(None, Some(vec![ov]));
        BoxKind::Attack.set_frame_override_meta_inherit(&mut f, 0);
        let updated = f.get_attack_override(0).expect("attack box exists");
        assert!(updated.meta.is_none());
        assert!(updated.hitbox.is_some());
    }

    #[test]
    fn box_kind_push_frame_override_box_attack_inherits_hitbox_with_default_meta() {
        // ユーザーの典型ユースケース: [Add Box] 直後は hitbox=None (sprite 継承)、
        // meta=Some(default) (上書きモードで即 damage 入力可能)。
        let mut f = frame_with_overrides(None, None);
        BoxKind::Attack.push_frame_override_box(&mut f, HitBox::new(0, 0, 16, 16));
        let v = f.attack_box_overrides.as_ref().expect("overrides exist");
        assert_eq!(v.len(), 1);
        assert!(
            v[0].hitbox.is_none(),
            "default Add Box should inherit hitbox"
        );
        assert_eq!(v[0].meta, Some(AttackBoxMeta::default()));
    }
}
