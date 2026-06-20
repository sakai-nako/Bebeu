use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlipMode {
    Horizontal,
    Vertical,
    Both,
}

/// 攻撃の Knockback ベクトル (px/s)。VelY+ で上向き、VelZ+ で手前 (ADR-0026)。
/// 攻撃側 AttackBoxMeta が保持し、被弾側で resistance を掛けて movement.State.VelX/Y/Z に充填される。
///
/// フィールド名は `vel_x` / `vel_y` / `vel_z` で揃える (YAML スキーマと engine 側 KnockbackVec
/// との対称性を保つため、`x` / `y` / `z` 単独より明示的)。clippy::struct_field_names は本意なので
/// 抑止する。
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

/// AttackBox に付随する攻撃情報。HitBox が幾何 (どこに当たり判定があるか) を担うのに対し、
/// AttackBoxMeta は「当たったら何が起こるか」(damage, hitstun 延長, Knockback ゲージ減算, 吹っ飛びベクトル) を担う。
///
/// 受け側 Character.physics.knockback_resistance で knockback / knockback_damage は減衰する。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AttackBoxMeta {
    /// HP 減算量 (デフォルト 0 = ダメージなし)。
    pub damage: u32,
    /// Knockback ゲージから減算するポイント。0 以下になると吹っ飛びに移行。
    pub knockback_damage: u32,
    /// ActionHit Animation 長に上乗せする硬直 (ms)。0 で Animation 長のみ。
    pub hitstun_extra_ms: u32,
    /// 吹っ飛び発動時に被弾側 movement.State.VelX/Y/Z に充填されるベクトル。
    pub knockback: KnockbackVec,
}

/// `Sprite.attack_boxes` / `Frame.attack_box_overrides` の要素。HitBox (幾何) と AttackBoxMeta (攻撃情報) を 1 つにまとめる。
///
/// **旧 YAML 互換**: 旧形式 (`{ top_left, bottom_right, depth }` を直接) を deserialize 時に
/// 吸収する。Serialize 時は常に新形式 (`{ hitbox: {...}, meta: {...} }`) で出力する。
/// 旧形式は `meta` を `None` (Default::default = ダメージ無し) として読み込む。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "RawAttackBox")]
pub struct AttackBox {
    pub hitbox: HitBox,
    /// 攻撃情報。`None` で「ダメージ・Knockback なし」(= 旧 HitBox 単独形式と等価)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<AttackBoxMeta>,
}

/// `AttackBox` の deserialize 専用 untagged enum。新形式と旧形式の両方を読み分ける。
#[derive(Deserialize)]
#[serde(untagged)]
enum RawAttackBox {
    /// 新形式: `{ hitbox: {...}, meta: {...} }`。
    New {
        hitbox: HitBox,
        #[serde(default)]
        meta: Option<AttackBoxMeta>,
    },
    /// 旧形式: HitBox を直接 (`{ top_left, bottom_right, depth }`)。
    Legacy(HitBox),
}

impl From<RawAttackBox> for AttackBox {
    fn from(raw: RawAttackBox) -> Self {
        match raw {
            RawAttackBox::New { hitbox, meta } => Self { hitbox, meta },
            RawAttackBox::Legacy(hitbox) => Self { hitbox, meta: None },
        }
    }
}

impl AttackBox {
    /// HitBox 部分のみから AttackBox を作る (meta なし)。canvas 上の新規作成で使う。
    #[must_use]
    pub fn from_hitbox(hitbox: HitBox) -> Self {
        Self { hitbox, meta: None }
    }

    /// `meta` を含むかを返す (= ダメージ / Knockback / hitstun_extra のいずれかが非デフォルトか)。
    /// UI 表示で「meta 編集中」のマーカーを出すなどに使う。
    #[must_use]
    pub fn has_meta(&self) -> bool {
        self.meta.is_some_and(|m| m != AttackBoxMeta::default())
    }
}

/// `HitBox.depth` (= world Z 厚み) が None のとき、所属 Character の `depth` にフォールバックする
/// (→ ADR-0024)。`HitBox::resolved_depth(character_depth)` で実際の値が得られる。
///
/// depth は world Z (奥行き) の **全幅** を表し、世界座標では `[char.PosZ - depth/2, char.PosZ + depth/2]`
/// の対称区間として解釈される (engine 側 `ResolveWorldBoxes` と一致)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitBox {
    top_left: [i32; 2],
    bottom_right: [i32; 2],
    /// world Z 厚み (奥行き)。`None` なら所属 Character の `depth` にフォールバック。
    /// 0 を許容する (= 厚みゼロで原理的に当たらない、特殊ケース)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    depth: Option<u32>,
}

/// HitBox の 4 つの角座標スロット。プロパティパネルの number input から
/// 「どの座標を編集中か」を伝えるのに使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitBoxCorner {
    TopLeftX,
    TopLeftY,
    BottomRightX,
    BottomRightY,
}

impl HitBoxCorner {
    /// `<input title>` 等の説明用ラベル。
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::TopLeftX => "Top-left X",
            Self::TopLeftY => "Top-left Y",
            Self::BottomRightX => "Bottom-right X",
            Self::BottomRightY => "Bottom-right Y",
        }
    }
}

/// HitBox を canvas 上で resize する時にどのハンドル (4 隅 + 4 辺の中点) を掴んだか。
/// 各 variant は「動かす辺 / 角」を表し、`HitBox::resized` で dx/dy をどの座標に乗せるかが決まる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl HitBox {
    /// 2 つの角座標から HitBox を作成する。座標の順序は問わず、top_left ≤ bottom_right となるよう正規化される。
    /// `depth` は None で初期化される (= Character.depth にフォールバック)。
    #[must_use]
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self::new_with_depth(x1, y1, x2, y2, None)
    }

    /// `new` と同様に正規化しつつ `depth` も指定する。
    #[must_use]
    pub fn new_with_depth(x1: i32, y1: i32, x2: i32, y2: i32, depth: Option<u32>) -> Self {
        let (x1, x2) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        let (y1, y2) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        Self {
            top_left: [x1, y1],
            bottom_right: [x2, y2],
            depth,
        }
    }

    #[must_use]
    pub fn top_left(&self) -> [i32; 2] {
        self.top_left
    }

    #[must_use]
    pub fn bottom_right(&self) -> [i32; 2] {
        self.bottom_right
    }

    /// `depth` の生値。`None` なら呼び出し側で Character.depth にフォールバックする。
    #[must_use]
    pub fn depth(&self) -> Option<u32> {
        self.depth
    }

    /// Character.depth を fallback として、本 HitBox の有効 depth (= 実際に使う厚み) を返す。
    /// `self.depth` が `Some` ならそちら、`None` なら `character_depth`。
    #[must_use]
    pub fn resolved_depth(&self, character_depth: u32) -> u32 {
        self.depth.unwrap_or(character_depth)
    }

    /// `depth` のみを差し替えた新しい HitBox を返す。
    /// 履歴記録の責務は呼び出し側 (no-op の同値判定もそちらで行う)。
    #[must_use]
    pub fn with_depth(&self, depth: Option<u32>) -> Self {
        Self {
            top_left: self.top_left,
            bottom_right: self.bottom_right,
            depth,
        }
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        self.bottom_right[0] - self.top_left[0]
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.bottom_right[1] - self.top_left[1]
    }

    /// 全体を平行移動した新しい HitBox を返す。`depth` は保持される。
    #[must_use]
    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        Self::new_with_depth(
            self.top_left[0] + dx,
            self.top_left[1] + dy,
            self.bottom_right[0] + dx,
            self.bottom_right[1] + dy,
            self.depth,
        )
    }

    /// 指定したハンドルだけ (dx, dy) 移動した新しい HitBox を返す。
    /// 角ハンドルは x/y 両方、辺ハンドルは片方の座標だけを動かす。`HitBox::new` で
    /// 正規化されるので、辺がもう片方を超えた時 (= 反転) は自動で top_left/bottom_right
    /// が入れ替わる。`depth` は保持される (XY plane の操作なので Z には影響しない)。
    #[must_use]
    pub fn resized(&self, handle: ResizeHandle, dx: i32, dy: i32) -> Self {
        let mut x1 = self.top_left[0];
        let mut y1 = self.top_left[1];
        let mut x2 = self.bottom_right[0];
        let mut y2 = self.bottom_right[1];
        match handle {
            ResizeHandle::TopLeft => {
                x1 += dx;
                y1 += dy;
            }
            ResizeHandle::Top => {
                y1 += dy;
            }
            ResizeHandle::TopRight => {
                x2 += dx;
                y1 += dy;
            }
            ResizeHandle::Left => {
                x1 += dx;
            }
            ResizeHandle::Right => {
                x2 += dx;
            }
            ResizeHandle::BottomLeft => {
                x1 += dx;
                y2 += dy;
            }
            ResizeHandle::Bottom => {
                y2 += dy;
            }
            ResizeHandle::BottomRight => {
                x2 += dx;
                y2 += dy;
            }
        }
        Self::new_with_depth(x1, y1, x2, y2, self.depth)
    }

    /// 各座標を倍率でスケールした新しい HitBox を返す。round で i32 に丸める。
    /// scale = 1.0 のときは数学的に同値（ただし round 経由なので元と一致する）。
    /// 座標範囲は数千 px 程度で truncation の懸念はないので allow する。
    ///
    /// `depth` は **スケールしない** で保持する。再 import 時の倍率はスプライト画像の
    /// 解像度変更に伴う XY ピクセル数の変化を表すもので、world Z 軸の厚みとは独立。
    /// Z 厚みを変えたい場合は明示的に `with_depth` で書き換える。
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn scaled(&self, scale: f64) -> Self {
        let scale_coord = |v: i32| (f64::from(v) * scale).round() as i32;
        Self::new_with_depth(
            scale_coord(self.top_left[0]),
            scale_coord(self.top_left[1]),
            scale_coord(self.bottom_right[0]),
            scale_coord(self.bottom_right[1]),
            self.depth,
        )
    }

    /// 1 つの角座標を `value` で更新した新しい HitBox を返す。`HitBox::new` で正規化されるので
    /// 入力の order はそのまま保たれる必要はない。値が変わらなければ呼び出し側が同値判定で
    /// 早期 return できるよう、本メソッドは履歴記録の責務は負わない。`depth` は保持される。
    #[must_use]
    pub fn with_corner(&self, corner: HitBoxCorner, value: i32) -> Self {
        let mut tl = self.top_left;
        let mut br = self.bottom_right;
        match corner {
            HitBoxCorner::TopLeftX => tl[0] = value,
            HitBoxCorner::TopLeftY => tl[1] = value,
            HitBoxCorner::BottomRightX => br[0] = value,
            HitBoxCorner::BottomRightY => br[1] = value,
        }
        Self::new_with_depth(tl[0], tl[1], br[0], br[1], self.depth)
    }

    /// 指定 pivot を中心に反転した新しい HitBox を返す。
    /// `depth` は保持される (反転は XY plane の操作で Z 軸には影響しない)。
    #[must_use]
    pub fn flipped_around(&self, pivot: [i32; 2], flip_mode: FlipMode) -> Self {
        let [x1, y1] = self.top_left;
        let [x2, y2] = self.bottom_right;
        let [px, py] = pivot;

        match flip_mode {
            FlipMode::Horizontal => {
                Self::new_with_depth(2 * px - x2, y1, 2 * px - x1, y2, self.depth)
            }
            FlipMode::Vertical => {
                Self::new_with_depth(x1, 2 * py - y2, x2, 2 * py - y1, self.depth)
            }
            FlipMode::Both => Self::new_with_depth(
                2 * px - x2,
                2 * py - y2,
                2 * px - x1,
                2 * py - y1,
                self.depth,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_doubles_each_coord_with_round() {
        let hb = HitBox::new(2, 4, 5, 9);
        let s = hb.scaled(2.0);
        assert_eq!(s.top_left(), [4, 8]);
        assert_eq!(s.bottom_right(), [10, 18]);
    }

    #[test]
    fn scaled_with_one_returns_equivalent_box() {
        let hb = HitBox::new(3, 7, 12, 20);
        let s = hb.scaled(1.0);
        assert_eq!(s.top_left(), hb.top_left());
        assert_eq!(s.bottom_right(), hb.bottom_right());
    }

    #[test]
    fn scaled_rounds_half_to_even_or_away_per_round() {
        // f64::round は「.5 で 0 から離れる方向」に丸める
        let hb = HitBox::new(1, 1, 3, 3);
        let s = hb.scaled(0.5);
        // 0.5, 0.5, 1.5, 1.5 → 1, 1, 2, 2
        assert_eq!(s.top_left(), [1, 1]);
        assert_eq!(s.bottom_right(), [2, 2]);
    }

    #[test]
    fn scaled_handles_negative_coords() {
        let hb = HitBox::new(-4, -2, 4, 2);
        let s = hb.scaled(1.5);
        // -6, -3, 6, 3
        assert_eq!(s.top_left(), [-6, -3]);
        assert_eq!(s.bottom_right(), [6, 3]);
    }

    #[test]
    fn new_initializes_depth_as_none() {
        let hb = HitBox::new(0, 0, 10, 10);
        assert_eq!(hb.depth(), None);
    }

    #[test]
    fn resolved_depth_falls_back_to_character_depth_when_none() {
        let hb = HitBox::new(0, 0, 10, 10);
        assert_eq!(hb.resolved_depth(20), 20);
    }

    #[test]
    fn resolved_depth_uses_explicit_value_when_some() {
        let hb = HitBox::new(0, 0, 10, 10).with_depth(Some(8));
        assert_eq!(hb.resolved_depth(20), 8);
    }

    #[test]
    fn translated_preserves_depth() {
        let hb = HitBox::new(0, 0, 10, 10).with_depth(Some(8));
        let moved = hb.translated(3, 4);
        assert_eq!(moved.depth(), Some(8));
    }

    #[test]
    fn scaled_does_not_change_depth() {
        let hb = HitBox::new(0, 0, 10, 10).with_depth(Some(8));
        let s = hb.scaled(2.0);
        assert_eq!(s.depth(), Some(8));
    }

    #[test]
    fn with_corner_preserves_depth() {
        let hb = HitBox::new(0, 0, 10, 10).with_depth(Some(8));
        let updated = hb.with_corner(HitBoxCorner::TopLeftX, 5);
        assert_eq!(updated.depth(), Some(8));
    }

    #[test]
    fn flipped_around_preserves_depth() {
        let hb = HitBox::new(0, 0, 10, 10).with_depth(Some(8));
        let f = hb.flipped_around([5, 5], FlipMode::Horizontal);
        assert_eq!(f.depth(), Some(8));
    }
}
