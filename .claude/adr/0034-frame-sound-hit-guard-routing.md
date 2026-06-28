# ADR-0034: Frame.sound の Hit / Guard 出し分け

## Status

Accepted (2026-06-26 に ADR-0019 の Frame.sound dispatch を拡張)

## Context

ADR-0019 で「SE は Animation Frame に紐づく event として `SoundGroup.number` を発火する」
方式を採用し、ADR-0019 を engine 側 (Rust + Bevy) に移植した直後 (2026-06-25)、次の演出要求が出た:

- **同じ攻撃 swing 内で、結果ごとに別の SE を鳴らしたい**
    - 空振り (Miss) → 風切り音だけ
    - ヒット (Hit) → 風切り音 + 打撃ヒット音
    - ガード (Guarded) → 風切り音 + 弾かれた金属音
- **振り音とヒット音を別フレームに置けるようにしたい** (例: frame 1 の構え時に風切り、frame 3 の
  振り切り時に打撃ヒット音)。「Hit/Guard を判定確定 tick に同 frame で鳴らす」だけだと
  AttackBox active 区間との順序が常に重なって演出に幅が出ない

ADR-0019 の `FrameSound { number, delay_ms }` だけでは:

1. 1 frame = 1 SoundGroup なので「結果で出し分け」を表現できない
2. attack result (Hit/Guard/Miss) を保持する場所が無い

なお `on_miss` も理屈上はあり得るが、現状「空振り音は frame 進入時の `number` で常時鳴らせば
代用できる」ため不要と判断し、Phase 1 では Hit / Guard の 2 種類だけに絞った。

## Decision

### `FrameSound` のスキーマを 3 系統 + delay に拡張する

```rust
pub struct FrameSound {
    /// 既定の Sound。frame 進入時、`AttackOutcome` が Idle (= まだ attack が成立していない)
    /// のとき、または `on_hit` / `on_guard` がそれぞれ None のとき フォールバック先として
    /// 選ばれる。「無条件で frame 進入時に latch したい音」全般 (= 攻撃の振り音、Hit voice、
    /// 通常時セリフ、loop 動作中の常時鳴らす音、など)。`None` で「default ライン無し」。
    pub number: Option<u32>,
    /// 直近の AttackBox が Hit していたら、frame 進入時にこちらを優先 latch する。`None`
    /// なら `number` にフォールバック (= 振り音をそのまま鳴らす)。
    pub on_hit: Option<u32>,
    /// 直近の AttackBox が Guard されていたら、frame 進入時にこちらを優先 latch する。
    /// `None` なら `number` にフォールバック。
    pub on_guard: Option<u32>,
    /// frame 進入から再生開始までの遅延 (ms)。3 系統共通。
    pub delay_ms: u32,
}
```

### `AttackOutcome` を anim-scope state として attacker に持たせる

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttackOutcome {
    #[default]
    Idle,      // まだ何も当てていない (or switchTo 直後)
    Hit,       // 通常 hit が成立した
    Guarded,   // ガードされた
}
```

- spawn 時に `AttackOutcome::default()` (= Idle) を attach
- `resolve_hits` の Hit 成立 / Guard 成立分岐で `*attack_outcome = Hit | Guarded` を書き込む
- `Changed<AnimationFrames>` (= switchTo) を捉えて `Idle` にリセット (新 attack の始まり)

### frame 進入時の選択ロジック

`step_dispatch` 内で、frame 進入を検知したら `AttackOutcome` と `FrameSound` から
**1 つだけ**の SoundGroup.number を選んで pending に latch する:

```rust
let chosen = match attack_outcome {
    AttackOutcome::Hit     => fs.on_hit.or(fs.number),
    AttackOutcome::Guarded => fs.on_guard.or(fs.number),
    AttackOutcome::Idle    => fs.number,
};
```

これにより:

- **frame 1: 振り音、frame 3: on_hit/on_guard** (別フレーム分離 case)
    - frame 1 進入: AttackOutcome=Idle → `number` 選択 → 風切り音
    - frame 2-3 で AttackBox active、Hit 成立 → AttackOutcome=Hit
    - frame 3 進入: AttackOutcome=Hit → `on_hit.or(number)` = on_hit → 打撃音
- **frame 3 のみで両方** (同フレーム case)
    - frame 3 進入直前: AttackOutcome=Idle (まだ resolve_hits 走っていない場合) → number
    - 同 tick の resolve_hits → AttackOutcome=Hit (system 順序で先に走る、後述)
    - 次 tick: prev==current で frame 進入扱いにならず latch されない (= 1 attack 1 sound)
- **空振り** (Miss)
    - frame 3 進入: AttackOutcome=Idle → number (frame 3 に `number: None` なら無発火)

### system 順序: `tick_sound_dispatch` は `resolve_hits` の後

attack resolution は `AttackSet::Resolve` SystemSet として export し、`tick_sound_dispatch`
を `.after(AttackSet::Resolve)` で順序固定する。これにより:

- 同 tick で frame が進入し、その frame の AttackBox が Hit を成立させる場合、
  `attack_outcome` の更新が `step_dispatch` の判定より前に確定する
- frame 3 進入と Hit 確定が同 tick なら、frame 3 の `on_hit` が即 latch される

## Alternatives Considered

- **Frame.sound に `on_miss` も追加**:
    - "空振り時に専用音 (例: 寒い無音風切り)" を表現できる
    - 空振り確定タイミング (= AttackBox active 区間が終わってから) の判定が要り、現状の
      "frame 進入時の状態だけで決める" モデルが崩れる
    - 振り音 (= `number`) を frame 進入時に常時鳴らせば、Miss は「振り音だけ」で表現できる
      ことが多く必須ではない。複雑さを避けて見送り
- **AttackBox 側に sound を持たせる (案 B)**:
    - 「同 frame の AttackBox 2 つに別音を当てたい」「frame と無関係に attack result で
      鳴らしたい」を表現できる
    - ADR-0024 の AttackBoxMeta との二重管理 + ADR-0019 の Frame.sound 体系と分離する
      ことになる
    - 現状そこまで細かな個別音は不要なので Frame.sound 拡張に寄せた
- **pending を queue 化 (FIFO 複数)**:
    - 同 frame に `number` + `on_hit` を両方 latch して両方鳴らせる
    - ADR-0019 で「1 スロット上書き」を選んだ判断と矛盾する (= 1 attack 1 sound 原則)
    - 必要になったら別 ADR で追加する

- **AttackOutcome を Combatant 内のフィールドに含める**:
    - Combatant は ADR-0024 で「被弾側の状態」を表現する component。attacker 視点の result
      を混ぜると semantic にズレる
    - 独立 component にしておけば、後で複数 player や AI 攻撃側に attach するのも簡単

## Consequences

**得られたもの**

- 振り音とヒット音/ガード音を別フレーム / 同フレームのどちらでも表現できる
- 既存 YAML (`sound: { number: 1 }`) は前方互換 (`on_hit` / `on_guard` 省略時 None)
- `number: None` で「on_hit / on_guard だけ持つフレーム」も表現できる (例: 振り切り frame に
  ヒット音だけ仕込む)
- 1 attack = 1 sound (1 スロット pending 原則) を維持

**支払うコスト**

- `FrameSound.number` が `u32` → `Option<u32>` に変わる (engine と editor の両方)
    - YAML 上は `skip_serializing_if = "Option::is_none"` で「書かれていない = None」
    - 旧 YAML (`number: 1`) は serde が `Some(1)` として読むので互換
- `AttackPlugin` が SystemSet (`AttackSet::Resolve`) を export する必要が出てきた
    - 既存の他 system が依存していないので追加コストは低い
- editor の frame_sound パネルが Swing / On hit / On guard の 3 セレクタに増える (UI 領域増)

**今後の拡張余地**

- `on_miss` を必要とする演出が出てきたら、AttackBox active 区間終了を検知する仕組みと
  あわせて別 ADR で追加できる (現スキーマに 1 field 追加で済む)
- AI / 他 player attacker にも `AttackOutcome` を attach すれば同じ仕組みが使える
- frame.sound に `on_critical` のようなより細分化された結果を追加する余地もあり、ADR-0034 の
  `match attack_outcome` パターンをそのまま拡張すれば良い
