# 操作方法

scaffolding 段階のため、現状はキーボード入力のみ対応。Gamepad / カメラ操作は未実装。

## Title scene

| キー | アクション |
|---|---|
| `Enter` / `Space` | Battle scene へ進む |

## Battle scene (Player 操作)

矢印キーと WASD は同義 (4 方向同時押し可)。

### 移動

| キー | アクション |
|---|---|
| `→` / `D` | 右へ移動 |
| `←` / `A` | 左へ移動 |
| `↑` / `W` | 奥へ移動 (Z 方向) |
| `↓` / `S` | 手前へ移動 (Z 方向) |

### 攻撃

| キー | アクション |
|---|---|
| `J` / `Space` | 通常攻撃 (立ちパンチ) |
| `K` | 下段攻撃 — 倒れた敵 (LieDown 状態) に当てる用の足元 AttackBox |
| `Jump` 中の `J` / `Space` | 空中攻撃 (JumpAttack) |

攻撃の damage / Knockback ベクトル / Guard 削り量は、Animation の `attack_box_overrides[].meta` で frame 単位に設定する。詳細は ADR-0024 / ADR-0028。

### ジャンプ・ガード

| キー | アクション |
|---|---|
| `I` | ジャンプ — `Physics.jump_velocity_y` で上昇、重力で落下、地面で `Idle` 復帰。ジャンプ中も `WASD` で空中移動可能 |
| `L` (押下継続) | ガード — `damage` / `knockback_gauge` を無傷化し、`guard_gauge` だけが削れる。離すと `Idle` |

二段ジャンプ・空中ガードは未対応 (`Jump` 中の `I` / `L` は無視される)。詳細は ADR-0027 / ADR-0028。

### 被弾とコンボの挙動

被弾は `Combatant.gauge` (Knockback ゲージ) を `AttackBoxMeta.knockback_damage` で削り、`0` 以下で吹っ飛び発動 → `KnockbackUp` → `KnockbackDown` → `Bounce` × N → `Slide` → `LieDown` → `Rise` → `Idle` のフローを物理駆動で進む (ADR-0024)。

連続コンボの暴発を防ぐ cap:

- `Physics.max_juggle_count` — 空中再被弾の上限。超えたら以降の airborne hit は **完全無敵** (素通り)
- `Physics.max_down_hit_count` — 倒れ中被弾 (DownHit) の上限。同様に完全無敵化

Guard 中に `guard_gauge` を削り切ると **GuardBreak** (ガードクラッシュ) が発動し、`Physics.guard_break_knockback` を充填して `KnockbackUp` に合流する (ADR-0028)。
