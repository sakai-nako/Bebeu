# ADR-0025: 吹っ飛びフローを単一 Action 列で表し、方向・致命傷を Animation 解決層で分離する

## Status

Accepted

## Context

ADR-0024 で吹っ飛びの物理ステージ (`KnockbackUp` / `KnockbackDown` / `BounceUp` / `BounceDown` / `Slide`) が入った。続いて以下の演出を入れたくなった:

1. **倒れる → 起き上がる** (`LieDown` → `Rise`): スライドが止まったあと、地面に倒れて一定時間後に起き上がる。
2. **死亡したら起き上がらない**: 致命傷を受けたら倒れたまま停止 (= KO 演出)。
3. **被弾方向で絵を変える**: 正面から殴られたら仰向けに、背後から殴られたら前のめりに飛ぶ。
4. **致命傷は専用の倒れ方**: 死亡時だけ違う倒れアニメ。

これらを素直に `Action` として持つと、`(物理ステージ 7) × (正面 / 背後 2) × (通常 / 致命傷 2)` で **最大 28 Action** に膨らむ。遷移ロジック (`Update` の switch と `Notify*`) も同じ倍率で組合せ爆発し、テストもメンテも破綻する。一方で「方向」も「致命傷」も**物理挙動は同一** (飛ぶ放物線・バウンド・スライドは方向や生死で変わらない)。変わるのは**再生する絵だけ**である。

加えて Stage 3a の死亡判定は `Action == ActionDead` 単独で行っていたが、死亡演出を Knockback フロー (浮いて落ちて倒れる) に乗せると、演出中の Combatant が `Action != ActionDead` の瞬間に「生きている」と誤判定されてしまう。

## Decision

**物理ステージ (Action) は被弾方向にも致命傷にも依存させない。方向・致命傷はフラグで保持し、引く Animation だけを解決層 (`Combatant.ResolveAnimation`) が多段フォールバックで出し分ける。死亡も同じ Action 列に乗せ、`LieDown` で永続停止させる。**

### Action は方向・生死で増やさない

Knockback フローの Action は 7 つ (`KnockbackUp/Down`, `BounceUp/Down`, `Slide`, `LieDown`, `Rise`) のまま。方向は `Combatant.HitFromBehind` (bool)、致命傷は `Combatant.FinalAction` (`ActionLieDown` か `ActionDead`) で保持する。どちらも吹っ飛び発動時に立て、フロー全体を通して保持する。

### Animation だけを 2 軸で解決する

`switchTo` は遷移先 Animation を `ResolveAnimation(action)` 経由で引く。被弾方向 × 致命傷の 2 軸で段階的にフォールバックする:

```
FinalAction==Dead && HitFromBehind → DeadBack 系 → Dead 系 → Back 系 → 通常系
FinalAction==Dead && 正面          →              Dead 系 →          通常系
通常              && HitFromBehind →                        Back 系 → 通常系
通常              && 正面          →                                  通常系
```

各段の `AnimationSet` スロットが nil なら次段へ。Role は prefix で命名する (`back_*` / `dead_*` / `dead_back_*`)。Dead 系・DeadBack 系に `Rise` は無い (死んだら起き上がらない)。**登録は全 optional** で、未登録なら順に劣化する (= 後方互換)。

### 死亡も同じ Action 列 + 永続停止

致命傷でも `ActionKnockbackUp` から同じフローを流す。`LieDown` に入ったとき `FinalAction == ActionDead` なら `Rise` に遷移せず、`is_loop=false` の Animation の最終 Frame で停止する (= 倒れたまま)。生死判定は `IsDead()` を **HP ベース** (`HP <= 0`) に統一し、`Action == ActionDead` 単独判定を廃止する。これで演出中も一貫して dead と判定でき、scene の KO 検知 (`resolveKO`) と整合する。

### LieDown / Rise の二重終了条件

`LieDown` / `Rise` は「Animation 終端」と「固定 timer」のどちらでも次段へ進む:

- Animation が `is_loop=false` → `Player.Finished()` で終了 (絵の尺がそのまま停止時間)
- Animation が `is_loop=true` / 未登録 → `switchTo` 時に `StageTimerMs = Physics.LieDownDurationMs / RiseDurationMs` を充填し、毎 tick 消化して 0 で次段

これで「倒れアニメを作り込んだキャラは尺どおり」「未整備のキャラも固定時間で破綻なく」起き上がれる。

### 被弾方向は scene が判定

`HitFromBehind` は scene が吹っ飛び発動後に立てる。被弾方向 (`dirX`) は `TakeHit` が攻撃側 FlipH で符号反転した**後**に確定するため、`statemachine` 単体では知り得ない。scene の `hitFromBehind(dirX, defenderFlipH)` が「被弾者が自分の正面方向に飛ぶ = 背中を押された = 背後被弾」と判定する。

## Alternatives Considered

- **方向 × 生死を Action として全部持つ (最大 28 Action)**:
    - 各状態が型として明示され、Animation 解決が `Action → Anim` の単純 lookup で済む
    - 遷移ロジックと `Notify*` が組合せ爆発。物理が同一なのに 4 倍の Action を保守することになり、テストが指数的に増える。却下

- **方向 / 生死を Animation の `variant` で表現**:
    - 既存の `(role, variant)` lookup に乗る
    - `variant` は combo system (attack の段) 用に予約済みで、意味が二重になる。`variant=1` が「背後被弾」なのか「2 段目の攻撃」なのか曖昧になる。prefix 付き role のほうが意味が一意

- **死亡を別 Action 列 / 別 state machine に切り出す**:
    - 生存フローと死亡フローが混ざらない
    - 吹っ飛びの物理 (放物線・バウンド・スライド) を二重実装することになる。`FinalAction` フラグ 1 個で分岐すれば物理は 1 系統で済む

- **フォールバックなし (全 Role を必須にする)**:
    - 解決ロジックが単純な lookup になる
    - 全 Character に 33 個の Animation を要求することになり、アセット制作が現実的でない。`dead_lie_down` 1 個から段階的に増やせる多段フォールバックが必須

- **`HitFromBehind` を statemachine 内で判定する**:
    - フラグ設定を 1 箇所 (`TakeHit`) に閉じられる
    - 被弾方向は攻撃側 FlipH 適用後の `dirX` に依存し、それは scene が `EffectiveKnockback` を充填する段で初めて確定する。statemachine に攻撃側の向きを渡す配線が増えるだけなので、scene 側で立てるのが素直

## Consequences

**得られたもの**

- Knockback フローの Action が 7 つに収まり、`Update` の switch と `Notify*` が線形・テスト可能なまま
- アセットを段階的に充実できる: `dead_lie_down` 1 個 → 方向別 → 致命傷別、と必要な分だけ足せる。未整備でも破綻しない
- 死亡演出が通常の吹っ飛び物理を再利用でき、`IsDead()` の HP 統一で scene の KO 検知と一貫
- `ResolveAnimation` が現在 Action と独立に「これから遷移する Action」の Animation を解決できる (`switchTo` から呼べる)

**支払うコスト / 注意点**

- `HitFromBehind` を scene が立てる責務を負う (statemachine 単体では被弾方向を知らない)。フラグ設定漏れは「常に正面被弾」として静かに劣化する
- `FinalAction` / `HitFromBehind` がフロー全体を通して保持される runtime 状態として増えた (Scene 単位でリセット)
- Animation の解決が「Action → 即 Anim」ではなく「Action + 2 フラグ → 多段フォールバック」になり、どの絵が出るかはフラグと登録状況の組合せで決まる (`buildAnimSet` の 33 スロットと `ResolveAnimation` の 4 段を併せて読む必要がある)
- 旧 `dead` Role は `dead_lie_down` に移管した (`normalizedRole` が読み替えるので既存 YAML は互換だが、新規は `dead_lie_down` を使う)

**今後の拡張余地**

- 被弾方向を 4 方向や任意角に増やす場合、軸を増やすとフォールバック段数が掛け算で増える (現状 2 軸 4 段)。3 軸目を足す前に「本当に絵を分ける必要があるか」を再検討する地点になる
- 受け身・起き上がり攻撃などは `LieDown` / `Rise` 中の入力判定を足す形で乗る (Action を増やさずに済む)
- editor 側で Role セレクタに prefix 付き Role を列挙済み (`entities/character/role.rs`) なので、新 Role 追加は engine `role.go` ↔ editor `role.rs` の 1:1 追加で閉じる
