# ADR-0027: ジャンプと空中攻撃を独立 State として導入し、Knockback と Y 軸物理を共有する

## Status

Accepted

## Context

ADR-0024〜0025 で吹っ飛びフロー (`KnockbackUp` → `KnockbackDown` → `Bounce*` → `Slide` → `LieDown` → `Rise`) と、それを駆動する Y 軸物理 (gravity / apex / landing) は実装済み。`Physics` には既に `jump_velocity_y` がデフォルト値を持ち、`Role` enum にも `Jump` が用意されていたが、`CharacterState` 側に Jump が無く、`movement::handle_input` も Y 速度を触る経路を持っていなかった。

Beat 'em up としては、

1. プレイヤーが任意にジャンプ → 落下し、空中で攻撃を当てられる
2. 空中攻撃は地上攻撃と別の判定枠 / 別 animation
3. 空中で hit を受けると ADR-0024 の「空中被弾 → 必ず浮く」経路に乗る

の 3 つを満たす必要がある。

## Decision

**Jump / JumpAttack を `CharacterState` の独立 variant として追加し、Y 軸物理は ADR-0024 の Knockback gravity と同じ system を共有する。**

### CharacterState 追加

```rust
enum CharacterState {
    // 既存: Idle, Walk, Attack, Hit, DownHit, DownAttack, KnockbackUp, ...
    Jump,        // 空中。Y 軸 gravity 適用、X/Z 入力で空中移動可
    JumpAttack,  // Jump 中の攻撃。Y 軸 gravity 継続、AttackBox 発火
}
```

- `is_locked()`: `Jump` は false (空中移動 / 攻撃を許可)、`JumpAttack` は true (攻撃終了か着地まで他入力ブロック)
- `to_role()`: `Jump → Role::Jump`、`JumpAttack → Role::JumpAttack`
- `end_oneshot_actions()`: Animation 終端で `Idle` に戻すのではなく、後述の **着地検出** で `Idle` 復帰させる

### 入力割当

| キー | 動作 |
|---|---|
| `I` | Jump (`pos.y == 0` でのみ受付、`vel_y = physics.jump_velocity_y` を充填) |
| `J` / `Space` | 地上 → Attack、Jump 中 → JumpAttack |
| `WASD` / 矢印 | 地上歩行と同じ X/Z 速度で空中移動可 |

二段ジャンプ・空中ガード・空中下攻撃はスコープ外。

### Y 軸物理の共有

`knockback::apply_gravity` (現状 `KnockbackUp/Down/BounceUp/Down` のみ対象) の対象に `Jump` / `JumpAttack` を加える。同 system が `vel_y -= gravity * dt`、`pos.y += vel_y * dt` を積分し、`pos.y <= 0` で着地。

着地検出 (`detect_landing`) で `Jump` / `JumpAttack` のときは:

- `Idle` に遷移
- `vel_y = 0`、`pos.y = 0` にクランプ
- (`JumpAttack` で attack hit-stop 中なら hit-stop 終了まで pose 維持してから Idle)

### JumpAttack の AttackBox

地上 Attack と同じく `frame.attack_box_overrides[].meta` で発火する。`JumpAttack` Animation の hit frame に攻撃側ベクトルを設定すれば、ADR-0024 の `resolve_hits` 経路がそのまま動く。地上 hit が `defender.grounded=true` 経路、空中 hit が `defender.grounded=false` 経路を取るのも既存挙動のまま。

## Alternatives Considered

- **JumpAttack を Attack の variant (`variant=1`) として表現**:
    - Role / state を増やさず animation 側で吸収できる
    - 入力分岐 (`is_in_air` で attack → jump_attack に切り替え) や `is_locked` の扱いがちらかる。State として独立させた方が遷移ロジックが 1 箇所に閉じる
- **Jump 用に新しい物理 system を切る**:
    - Knockback と Jump は意味的に別の挙動
    - 実体は「`vel_y` に重力をかけて積分し、`pos.y <= 0` で着地」しかなく、共通 system で十分。挙動差は「着地時にどの state へ落ちるか」だけで、それは `detect_landing` 内の match 1 行で吸収できる
- **二段ジャンプを最初から入れる**:
    - 表現の幅が広がる
    - 着地カウンタ・空中入力消費の追加状態が必要で、最初の MVP に対しては YAGNI。`Combatant` に `air_jumps_remaining` を足せば後付けできる

## Consequences

**得られたもの**
- ADR-0024 の Y 軸物理がそのまま流用でき、Jump 用に新規 system を増やさずに済む
- 空中攻撃が独立 state なので「Jump 中だけ別 hitbox」「JumpAttack 専用 hit-stop」を Animation データ側で完結して書ける
- 空中被弾は ADR-0024 の `!grounded → 浮かせ続ける` で自動的に空中コンボに接続される

**支払うコスト / 注意点**
- `is_locked` の真偽が state ごとに細かくなる (Jump=false / JumpAttack=true)。新 state を追加するときに `is_locked` を埋め忘れない規約が必要
- 空中移動の x/z 速度は地上と同じ式で動かす。`Physics` に「空中減速率」を入れて分離する設計余地は残しておくが MVP では不要

**今後の拡張余地**
- 二段ジャンプ: `Combatant.air_jumps_remaining` を Jump で消費・Land で復帰
- 空中ガード / 空中下攻撃: 本 ADR のパターン (State 追加 + 入力分岐) を踏襲
- ホップ / ハイジャンプ: `Physics.jump_velocity_y` を入力長押し / コマンドで補正
