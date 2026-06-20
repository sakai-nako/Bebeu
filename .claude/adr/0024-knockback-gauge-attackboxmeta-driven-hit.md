# ADR-0024: 被弾を Knockback ゲージ + AttackBoxMeta 駆動の吹っ飛びシステムにする

## Status

Accepted

## Context

Stage 3a の被弾処理 (`statemachine.TakeHit`) は「固定 damage で HP を引き、Hit アニメを出す」だけだった。Beat 'em up として以下を表現できていなかった:

1. **のけぞり (Hit) と吹っ飛び (打ち上げ) の区別**: 軽い攻撃はその場でのけぞるだけ、重い攻撃や連打のフィニッシュは相手を浮かせて打ち上げる、という強弱が無い。
2. **攻撃ごとの効果差**: ダメージ・吹っ飛び量・硬直がすべて Config 固定値で、技ごとに変えられない。
3. **空中コンボ**: 浮いている相手を追撃して浮かせ続ける挙動 (空中被弾は必ず浮く) が無い。
4. **キャラの重さ / 耐性**: 重量級は吹っ飛びにくい、軽量級は派手に飛ぶ、といったキャラ差が無い。

Stage 3a の attack box は `HitBox` (幾何のみ) で、「当たったら何が起こるか」のデータを持たない。また「無敵フレーム」は当時 `ActionHit` / `ActionDead` 中の `TakeHit` を no-op にすることで表していたが、これは「のけぞり中は一切当たらない」という硬直した規約で、コンボ (のけぞり中の追撃) と両立しない。

## Decision

**被弾を「Knockback ゲージ」と「攻撃ごとの `AttackBoxMeta`」で駆動し、ゲージ枯れ・空中被弾・致命傷のいずれかで吹っ飛びを発動する。吹っ飛びの物理 (打ち上げ → 落下 → バウンド → スライド) は速度ベクトルで表し、積分は scene が行う。**

### AttackBoxMeta — 攻撃ごとの効果データ

`AttackBox` を `HitBox` (幾何) + `AttackBoxMeta` (効果) のペアにする (`shared/hitbox/attack_box.go`):

```go
type AttackBoxMeta struct {
    Damage          uint32       // HP 減算
    KnockbackDamage uint32       // Knockback ゲージ減算
    HitstunExtraMs  uint32       // Hit アニメ長に上乗せする硬直
    Knockback       KnockbackVec // 吹っ飛び発動時に被弾側へ充填する速度 (vel_x/y/z)
}
```

`KnockbackVec.VelX` は「攻撃側の前方向を +」とする座標系で書く (scene が攻撃側 FlipH で符号を反転、ADR-0023)。

### Knockback ゲージ

`Combatant` に `KnockbackGauge` を持たせる。`Physics.KnockbackThreshold` で初期化し、被弾で `KnockbackDamage` ぶん削る。`0` 以下になったら吹っ飛び発動。Hit を受けると `GaugeRecoveryTimer = Physics.HitRecoveryMs` が立ち、消化しきると full 回復する。= 短時間に集中して削れば吹っ飛び、間隔が空けば回復する、という「のけぞり↔吹っ飛び」の境界をゲージ 1 本で表す。

### 吹っ飛び発動条件

`TakeHit` 内で次のいずれかなら発動:

- `KnockbackGauge <= 0` (削りきった)
- `!defenderGrounded` (空中被弾 → 必ず浮かせ続ける)
- `lethal` (致命傷 = HP 0)

### resistance による減衰

`Physics.KnockbackResistance` (0..1) で被軽減する。`attenuation = 1 - clamp(resistance, 0, 1)` を Damage / KnockbackDamage / Knockback ベクトルすべてに掛ける。重量級は resistance を上げる。

### 吹っ飛びの物理フロー

発動すると `ActionKnockbackUp` に遷移し、`TakeHit` は `HitResult{KnockbackTriggered, EffectiveKnockback, LethalHit}` を返す。**`Combatant` は速度を持たない**ので、scene が `state.VelX/Y/Z` を `EffectiveKnockback` で充填する。以後の積分と段階遷移は scene が担う:

- `KnockbackUp/Down`: `ApplyVertical` (gravity) + `ApplyHorizontal` (摩擦 0 = 慣性)。頂点 (VelY≤0) / 着地 (IsGrounded) を検知して `Notify*` を呼ぶ。
- **Bounce**: 着地時に `RemainingBounces > 0` なら、VelY を反転して `Physics.BounceDampening` 倍、VelX/VelZ も dampening して `BounceUp` へ。残数を 1 消費。`MaxBounceCount` 回まで跳ねる。
- **Slide**: 残数 0 で着地したら `Slide` へ。`Physics.GroundFriction` で X/Z を減速し、ほぼ停止で次段。

物理ステージ自体の状態遷移と、その後の LieDown / 死亡演出・方向別 Animation は ADR-0025 が扱う。

### 無敵フレームを BodyBox-driven に変更

旧「Hit/Dead 中の `TakeHit` no-op」を撤廃し、**defender の BodyBox 解決結果が空なら hit を弾く**規約に統一する (`attack.ResolveHitMeta`)。無敵タイミングは Frame の `body_box_override` を Disable にすることでアニメ作家が表現する。これでコンボ (のけぞり中の追撃) が表現可能になる。

### 後方互換

`AttackBox` の YAML は新形式 `{hitbox, meta}` と旧形式 (HitBox 直書き) を両対応 (`UnmarshalYAML`)。旧形式は `Meta=nil`。さらに scene は `meta == nil || (Damage==0 && KnockbackDamage==0)` のとき Config の固定値で fallback する。= Phase 4 以前の attack yml も従来通り damage が入る。

## Alternatives Considered

- **HP 閾値方式 (残 HP の割合で吹っ飛び)**:
    - データが増えない
    - 「体力」と「吹っ飛び耐性」が結合する。瀕死だと常に吹っ飛ぶ / 満タンだと絶対吹っ飛ばない、という不自然さ。別ゲージにすれば「硬いが体力は低い」等を独立に設計できる

- **ヒット数カウント (N 発当てたら吹っ飛び)**:
    - 実装が単純
    - 攻撃の強弱を表現できない (弱パンチ 1 発と必殺技 1 発が同じ重み)。`KnockbackDamage` を技ごとに変える現案のほうが表現力が高い

- **速度ベクトルを `Combatant` に持たせる**:
    - `TakeHit` 内で完結し、scene が `HitResult` を見て充填する手間が無い
    - 位置 / 速度が `movement.State` と `Combatant` に二重管理になる。movement に一元化し、`Combatant` は「状態 (Action) のみ」に保つほうが責務が明快 (統合テストも movement 側に集約できる)

- **吹っ飛び全体を別 state machine に切り出す**:
    - 通常の Action 遷移と混ざらない
    - scene の物理積分との往復 (`Notify*`) が二重配線になる。`statemachine.Action` の一系列に乗せ、物理だけ scene に出すほうが配線が 1 系統で済む

- **無敵を Action ベース (Hit 中 no-op) のまま残す**:
    - 実装変更不要
    - コンボ (のけぞり追撃) と原理的に両立しない。BodyBox-driven なら「この frame だけ無敵」の粒度で制御でき、editor の 3-state box override (ADR-0014) と仕組みを共有できる

## Consequences

**得られたもの**

- 攻撃 yml に `meta` を足すだけで「弱パンチはのけぞり、強キックは打ち上げ、必殺技は多段バウンド」を技ごとに設計できる
- `KnockbackResistance` / `BounceDampening` / `MaxBounceCount` 等の `Physics` でキャラの重さ・吹っ飛び方を独立に調整できる
- 空中コンボ (浮いた相手を追撃) が `!defenderGrounded → 必ず吹っ飛び` で自然に成立
- 無敵が frame 単位 (BodyBox Disable) になり、コンボと無敵の両立、editor の box override 仕組みとの共有が得られた
- 旧 attack yml / physics 無し YAML はそのまま動く (fallback + ApplyDefaults)

**支払うコスト / 注意点**

- `Combatant` に非永続の runtime 状態が増えた (`KnockbackGauge` / `GaugeRecoveryTimer` / `RemainingBounces` 等)。Scene 単位でのリセット漏れに注意
- scene が `HitResult.KnockbackTriggered` を見て `VelX/Y/Z` を充填する責務を負う。`TakeHit` 単体では吹っ飛びが「始まらない」(状態と物理が分離している副作用)
- `Physics` パラメータが 8 個に増え、キャラごとの tuning コストが上がる。`ApplyDefaults` で 0 値を既定に補正するが、`BounceDampening=0` を「跳ねない」意図で設定したい場合は `0.0001` 等を使う fail-soft (0 は default 扱いになるため)
- VelY 反転やバウンド計算など物理の数値は `scenes/battle` に散る。statemachine 側は状態のみなので、挙動を追うときは両方を読む必要がある

**今後の拡張余地**

- combo system: `AttackBoxMeta` に combo scaling (多段ヒットで damage / knockback を逓減) を足す層を `TakeHit` の手前に挟める
- 受け身 (空中で入力すると bounce をキャンセル) は `NotifyKnockbackBounced` の手前に入力判定を足す形で乗る
- ゲージの可視化 (HUD) は `Combatant.KnockbackGauge` / `KnockbackThreshold` を読むだけで実装できる
