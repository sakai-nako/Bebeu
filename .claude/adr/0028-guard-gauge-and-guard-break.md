# ADR-0028: Guard を gauge 1 本で表現し、GuardBreak は Knockback フローに合流させる

## Status

Accepted

## Context

ADR-0024 で被弾を `knockback_gauge` で管理する仕組みが入った。プレイヤーが任意に防御 (ガード) する選択肢はまだ無く、攻撃を受けると必ず `Hit` or `Knockback*` に遷移していた。

Beat 'em up に最低限求められるガード機能:

1. プレイヤー入力で「ガード」状態に入る (押下継続中)
2. ガード中に被弾しても `damage` / `knockback_gauge` は無傷
3. ただしガードには上限があり、削られ続けると「ガードクラッシュ」で大きな硬直 + 吹っ飛び
4. 攻撃側は技ごとに「ガード削り量」を別パラメータで設定できる

これを **既存 `knockback_gauge` と別の `guard_gauge` を 1 本足し、break 後は ADR-0024 の Knockback フローへ合流させる** ことで、新規物理コードを増やさずに表現する。

## Decision

**`Combatant` に `guard_gauge` を 1 本追加し、ガード中の被弾はこのゲージのみを削る。`guard_gauge <= 0` で `GuardBreak` 状態に遷移し、1 frame で `KnockbackUp` へ合流して既存の吹っ飛びフローに乗せる。**

### CharacterState 追加

```rust
enum CharacterState {
    // ...
    Guard,       // 押下継続中。地上で攻撃を受けても damage / knockback_gauge は無傷
    GuardBreak,  // guard_gauge 枯渇で 1 frame 遷移、即 KnockbackUp に流れる中継点
}
```

- `is_locked()`: `Guard` は false (移動はロックしないが、攻撃入力は別途禁止)、`GuardBreak` は true
- `to_role()`: `Guard → Role::Guard`、`GuardBreak → Role::GuardBreak`
- 入力 `L` 押下中で `Guard` 維持、離したら `Idle`

### AttackBoxMeta 拡張

```rust
struct AttackBoxMeta {
    // 既存: damage, knockback_damage, knockback (KnockbackVec), hit_stop
    guard_damage: u32,  // Guard 中に当たったとき guard_gauge を削る量
}
```

`guard_damage = 0` の攻撃は「ガード不可能 / ガード貫通」相当 (gauge を削らず素通り) ではなく、本 ADR では **`guard_damage = 0` でも guard 中の damage は無効化** する。「ガード不能技」は将来 `guard_break_only: bool` 等の別 field を追加する余地として残す。

### Physics 拡張

```rust
struct Physics {
    // 既存: knockback_threshold, hit_recovery_ms, ...
    guard_break_threshold: u32,    // guard_gauge の初期値 / max
    guard_recovery_ms: u32,        // 最後にガード被弾してから何 ms で guard_gauge full 回復するか
    guard_break_knockback: KnockbackVec, // GuardBreak 発動時に充填する吹っ飛びベクトル
}
```

### Combatant 拡張

```rust
struct Combatant {
    // 既存: gauge (knockback_gauge), gauge_recovery_remaining_ticks, ...
    guard_gauge: i32,
    guard_recovery_remaining_ticks: u32,
}
```

### Hit 解決ロジック (`resolve_hits` 内)

```text
if defender.state == Guard {
    combatant.guard_gauge -= meta.guard_damage as i32;
    combatant.guard_recovery_remaining_ticks = ms_to_ticks(physics.guard_recovery_ms);
    if combatant.guard_gauge <= 0 {
        defender.state = GuardBreak;  // 次フレームで KnockbackUp に転換
        kinematic_vel = physics.guard_break_knockback (攻撃側 facing で X 符号反転);
    }
    // damage / knockback_gauge は無傷
    return Guarded;
}
// 通常被弾は ADR-0024 経路 (省略)
```

### GuardBreak → KnockbackUp 転換

`GuardBreak` は 1 frame の中継 state。次フレームの state machine tick で `KnockbackUp` に遷移させる。`kinematic_vel` には前 frame で充填済みの `guard_break_knockback` が乗っているので、以後は ADR-0024 の吹っ飛び物理がそのまま駆動する。Animation 上は `Role::GuardBreak` (盾が砕ける表現) を 1〜数 frame 見せ、その後 `Role::KnockbackUp` に切り替わる。

### guard_gauge の自然回復

ADR-0024 の `gauge_recovery_remaining_ticks` と同型で、ガード被弾後 `physics.guard_recovery_ms` をカウントダウンし、0 で `guard_gauge` を full に戻す。連続ガードで削れる / 間が空けば回復する、という挙動を gauge 1 本で吸収する。

### 空中ガード

未サポート。`Jump` / `JumpAttack` 中の `L` 入力は無視する (ADR-0027 と同じ MVP スコープ判断)。

### Role rename

既存 `Role::Block` を `Role::Guard` に rename し、YAML の `role: block` も `role: guard` に統一する。`Role::JumpAttack` / `Role::GuardBreak` を新規追加 (Back/Dead prefix も同様に展開)。

## Alternatives Considered

- **ガードを `Hit` の damage = 0 で表現**:
    - 既存経路を使い回せる
    - ガード固有の「上限」「削り量」「クラッシュ」を表現する場所が無く、結局別 field を増やすことになる。state を独立させた方が `is_locked` / animation / 入力分岐がきれいに分かれる
- **ガード中は damage を割合減算する (例: 0.3 倍)**:
    - 「ガードしても少し痛い」表現が可能
    - HP / knockback_gauge と guard_gauge が同時に削れて、break タイミングと致死タイミングが交錯する。MVP では「ガード成立中は無傷、break で初めて崩れる」一刀両断モデルが分かりやすい
- **GuardBreak を独立した吹っ飛び物理として実装**:
    - ガードクラッシュ専用モーションを完全に独自設計できる
    - ADR-0024 の吹っ飛びフロー (Bounce / Slide / LieDown / Rise) を二重実装することになる。`guard_break_knockback` ベクトルだけ差し替えて既存物理を流用すれば、表現の差は Animation / `Physics` 数値で十分出せる
- **ガード方向の概念 (中段 / 下段)**:
    - 攻防の駆け引きが深くなる
    - MVP では全攻撃ガード可能。将来 `AttackBoxMeta.guard_kind: Mid|Low|Unblockable` 等を追加できる余地として残す

## Consequences

**得られたもの**
- ADR-0024 の吹っ飛び物理を完全に再利用でき、GuardBreak 用に新規物理コードが要らない
- `AttackBoxMeta.guard_damage` を技ごとに設定できるので「弱攻撃はガードしても削らない / 必殺技は 2 発で割れる」を YAML で設計可
- gauge 1 本 / 自然回復モデルなので、ADR-0024 の `knockback_gauge` と完全に同型 (実装パターンを再利用)

**支払うコスト / 注意点**
- `Combatant` に runtime state が 2 個増える (`guard_gauge` / `guard_recovery_remaining_ticks`)。scene 単位のリセット漏れに注意
- `Physics` パラメータが 3 個増え、キャラごとの tuning コストが上がる。ApplyDefaults で 0 値を既定に補正する
- `Role::Block` → `Role::Guard` rename で sample-projects の YAML が変更必要 (block.yml → guard.yml)。本 ADR 採用時点では Block ロールを参照する YAML が無いので影響は局所
- `GuardBreak` の 1 frame 中継は state machine 上の "暗黙の自動遷移" になる。読みづらさを減らすため `state_machine.rs` の遷移ロジックに集約して書く

**今後の拡張余地**
- ガード不能技: `AttackBoxMeta.guard_break_only: bool` を追加
- 中段 / 下段: `AttackBoxMeta.guard_kind` + `Guard` 入力方向 (`L+S` で下段ガード等)
- ジャストガード: ガード入力直後 N frame の被弾を「gauge を削らず逆に攻撃側を硬直」させる、`Combatant.guard_just_window_ticks` で表現可能
- ガード可視化 (HUD): `Combatant.guard_gauge` / `Physics.guard_break_threshold` を読むだけで HUD 化できる
