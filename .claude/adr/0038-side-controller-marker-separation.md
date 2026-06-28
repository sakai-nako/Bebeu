# ADR-0038: キャラクター marker を Side / Controller 2 軸に分離する

## Status

Accepted (2026-06-28、Issue #6 で実装、MR !? でマージ予定)

## Context

ADR-0035 Phase 1-3 で 3 種類の AI Brain (`MeleeBrain` / `AllyBrain` / `BotBrain`) を導入した結果、
character marker が `Player(PlayerId)` / `Enemy` / `Ally` の 3 種類になった。それぞれが
「**陣営 (side)**」と「**操作主体 (controller)**」を 1 marker で兼ねていて、Phase 2 / 3 で
2 つの歪みが顕在化:

1. **Enemy → Ally の chase / damage 非対称**
   - `MeleeBrain.target` query が `With<Player>` のみ → Enemy は Ally を視認・追跡しない
   - `attack.rs` の resolve 経路が **Player → Enemy / Enemy → Player / Ally → Enemy の 3 系統**
     → Enemy → Ally の系統が無い
   - 3 marker のまま継ぎ足し続けると、victim が増えるたびに `resolve_*_attacks_on_*` を
     追加する増殖パターンになる
2. **`AiConfig::Bot(BotConfig)` YAML 化 が遅延**
   - Phase 3 で BotBrain は env var `BEATEMUP_PLAYER_BOT` 専用 (`AiConfig` enum に Bot variant 追加せず)
   - character YAML から AI を切り替える表現を Side / Controller 軸が直交化したあとで
     `Controller` 軸を YAML から指定する形にする方が筋が良い (Issue #5 note_82 で decide)

ADR-0035 §「ランタイムすり替え対応 (段階的)」で `Side: Hero | Villain` の方向性は
既に予告されていた:

> 完全すり替え対応は Phase 4+ で marker 分離 ADR として別途扱う。設計の方向性は決めておく:
> - `Side: Hero | Villain` (= 当たり判定 / 勝敗の所属) を全 character に attach
> - 「操作主体」は Brain component の種類 ... で表現

本 ADR はこの予告を実装する Phase 4 の決定。3 marker を 2 enum component に直交化し、
Enemy → Ally の chase / damage を runtime レベルで対称化し、`AiConfig::Bot` を YAML 化する。

## Decision

### 4 つの変更を同時に入れる

| 変更 | 対象 | 役割 |
|------|------|------|
| `Side: Hero \| Villain` enum component | engine ECS | キャラクターの陣営。攻撃対象 / HUD target / Brain target の起点 |
| `Controller: Human \| Ai` enum component | engine ECS | 操作主体。`Human` は `PlayerInputController` が `AiCommand` を書く、`Ai` は Brain が書く |
| `PlayerId` を独立 component に分離 | engine ECS | 旧 `Player(PlayerId)` を解体、`PlayerId` 単体で `Controller::Human` 持ち entity に attach |
| `AiConfig::Bot(BotConfig)` variant + YAML 経路 | entities/character + scene | Hero character YAML の `ai: kind: bot` で Bot 化、env var との両立 |

### enum component 採用、ZST marker は使わない

`Side` / `Controller` は `#[derive(Component)] enum { ... }` で表現する。代替案の
「ZST marker 2x2 (HeroSide / VillainSide / HumanControlled / AiControlled の 4 marker)」は
Bevy の Query filter (`With<X>`) ベースの archetype 解析が効くメリットがあるが:

- 状態爆発はしない (Side だけで 2 通り、Controller だけで 2 通り)
- character spawn 時に 2 軸とも attach し忘れるリスクがある (= `Required` component で
  軽減できるが余計な複雑性)
- 値で読み取りたい code (HUD / Brain target / attack resolve の inner skip) では
  `&Side` を Query 内で取る方が semantic に読める

enum を採用するコストとして、attack resolve の attacker / victim Query の disjoint 解析は
**Brain marker (`MeleeBrain`) の有無**で代替する (= Villain は `MeleeBrain` 持ち、Hero は
持たない archetype に分かれる)。詳細は下記「attack resolve」節。

### 旧 marker は完全削除する

`Player(PlayerId)` / `Enemy` / `Ally` は削除する。互換 alias / Query trait は残さない。
理由: 並行運用すると mixed state が長期化し、本 Issue でやりたいクリーンアップが流れる。
本 Issue 内で全 file (`movement.rs` / `ai.rs` / `attack.rs` / `state_machine.rs` /
`state_debug.rs` / `hud.rs` / `scenes/battle.rs`) を一括書き換える。

`PlayerId` は独立 component として `Controller::Human` 持ち entity に直接 attach する
(ADR-0030 multi-player 設計の HUD `target: p1` 引きを破らない)。

### Brain target は反対 Side 全体に拡張

旧 `MeleeBrain` の target は `With<Player>` 限定だったが、本 ADR で `Side::Hero` 全体に
拡張:

| Brain | Self | target |
|---|---|---|
| `MeleeBrain` | Side::Villain + Controller::Ai | `Side::Hero` 全体 (= 旧 Player + 旧 Ally) |
| `AllyBrain` | Side::Hero + Controller::Ai | nearest `Side::Villain`、不在時 nearest `Side::Hero + Controller::Human` (Follow) |
| `BotBrain` | Side::Hero + Controller::Ai | nearest `Side::Villain` |

Brain owner の Side 判定は不要 (= Brain marker 持ち = 種類で確定)。target query は
`Without<<Brain>>` で self 除外、`&Side` で値判定して反対 Side に絞る。新 helper
`nearest_on_side` / `nearest_hero_human` を `ai.rs` に追加。

### attack resolve を 2 系統に統合

旧 3 系統 (`resolve_player_attacks` / `resolve_enemy_attacks` / `resolve_ally_attacks`) を
2 系統に統合する:

| system | attacker | victim |
|---|---|---|
| `resolve_hero_attacks` | `Side::Hero` (旧 Player + Ally) | `Side::Villain` (旧 Enemy) |
| `resolve_villain_attacks` | `Side::Villain` (旧 Enemy) | `Side::Hero` (旧 Player + Ally) |

これにより:
- Enemy → Ally の damage 経路が `resolve_villain_attacks` で自動的に処理される
  (= Phase 4 の主要動機の達成)
- 新 victim が増えても resolve system を増やさなくて済む (= 増殖パターンの解消)

**attacker / victim Query の disjoint は Brain marker `MeleeBrain` の有無**で取る:
- `resolve_hero_attacks`: attacker `Without<MeleeBrain>` (= Hero side は AllyBrain / BotBrain /
  Brain なし)、victim `With<MeleeBrain>` (= Villain)
- `resolve_villain_attacks`: attacker `With<MeleeBrain>`、victim `Without<MeleeBrain>`

`&Side` 値判定は **inner skip 用**で、Side enum だけでは archetype disjoint は作れないため
Brain marker を補助に使う。Hero / Villain で disjoint な archetype に分かれる前提
(= Hero に `MeleeBrain` を attach することは想定しない)。

`LastEngagedWith` (ADR-0031) は **`Controller::Human` 持ち attacker のみ**に attach し、
attack resolve では `Option<&mut LastEngagedWith>` で参照する。これにより HUD の
engagement-link 起点 (`enemy_hp_bar { last_engaged_by: p1 }`) は引き続き Player attack
でだけ書き込まれる (= ADR-0031 挙動の維持)。

victim 側の Hit role first frame fallback は:
- `Side::Villain` victim (= 旧 Enemy) → entity 持ち `EnemyAnimationSet`
- `Side::Hero + Controller::Human` victim (= 旧 Player) → `PlayerAnimationLibrary` Resource
- `Side::Hero + Controller::Ai` victim (= 旧 Ally) → entity 持ち `EnemyAnimationSet`

`resolve_villain_attacks` の victim_query で `Option<&EnemyAnimationSet>` を取り、Some なら
entity 持ち、None なら `PlayerAnimationLibrary` Resource にフォールバックする一行 chain で
両立する。

### `AiConfig::Bot(BotConfig)` 経路 (env var + YAML 両立)

`AiConfig` enum に `Bot(BotConfig)` variant を追加する:

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiConfig {
    Melee(MeleeConfig),
    Ally(AllyConfig),
    Bot(BotConfig),
}
```

`BotConfig` は Phase 3 の流用方針 (= `MeleeConfig` と同形パラメータ) を YAML に明示化した
data type。フィールドは `MeleeConfig` と完全同形 (chase_*/attack_*/cooldown/decision_interval/
min_dwell)。`BotConfig::into_melee_config()` で `BotBrain::new` の引数に詰め替える。

将来 Bot 専用 param (perception / panic / replay 等) が必要になったら BotConfig に追加する
余地はあるが、本 Issue では同形のまま導入 ([memory: dedup_later] 方針、Issue #7 の BrainConfig
共通基底で再評価)。

scene 側 (`battle.rs` Player spawn 経路) は **env var 優先 + 両立** で実装:
- env var `BEATEMUP_PLAYER_BOT` 非空 → `MeleeConfig::default()` で BotBrain attach (Phase 3 既存)
- env var 未指定 + YAML `ai: kind: bot` → YAML 由来 `BotConfig::into_melee_config()` で
  BotBrain attach
- 両方指定 → env var 優先 (= Phase 3 挙動の回帰なし、ADR-0035 Phase 3 補追規約を維持)

Hero character YAML に `ai: kind: melee` / `ai: kind: ally` が書かれていたら warn 出して
無視 (= hero は `null` か `bot` のみ受け付ける)。Villain character YAML に `ai: kind: bot` /
`ai: kind: ally` が書かれていたら warn 出して無視 (= villain は `melee` のみ)。

### state_machine.rs / state_debug.rs / hud.rs の filter

旧 marker 廃止に伴い:

- `sync_animation` (Player 用): `With<Player>` → `Without<EnemyAnimationSet>` (= entity 持ち
  library を持たない character、`PlayerAnimationLibrary` Resource を引く)
- `sync_enemy_animation` (Enemy / Ally 共用): `Or<With<Enemy>, With<Ally>>` → component の有無
  で disjoint (= filter 不要、`&EnemyAnimationSet` を取る query 自体が暗黙 filter)
- `state_debug.rs` の `targets` query: `Or<With<Player>, With<Enemy>, With<Ally>>` →
  `With<Side>` (= 全 character)
- `hud.rs` の `enemy_query: With<Enemy>` → `&Side` を取って inner skip で `Side::Villain` 限定
- `hud.rs` の `spawn_enemy_overhead_hp_bars` の `Added<Enemy>` → `Added<Side>` + 内側で
  `matches!(side, Side::Villain)` で skip
- `camera_follow` / `player_input_controller`: 旧 `With<Player>` → `Side::Hero + Controller::Human`
  の値判定。`PlayerInputController` は `Without<BotBrain>` 排他 (Phase 3 案 A) を維持。

### EnemyTag は名前のまま維持

`EnemyTag` (ADR-0031) は HUD の `enemy_hp_bar { tag: boss }` から参照される component で、
semantic に「Villain 側の HP bar ラベル」を指す。本 ADR で `Villain` に rename しない:
- `EnemyTag` のままで semantics が一貫している (= Villain side の HP bar のラベル)
- YAML スキーマ (`Character.tag`) と editor UI が `tag` を共有しているので機械的な
  rename はコストが高い (= Issue #8 editor UI 対応と合わせて再評価が筋)

## Alternatives Considered

- **ZST marker 2x2 (HeroSide / VillainSide / HumanControlled / AiControlled)**: Bevy schedule
  の archetype 解析が綺麗に効き、attack resolve の disjoint も `With<HeroSide>` /
  `With<VillainSide>` で自然に組める。ただし spawn 時の 2 軸 attach 漏れリスクと、`&Side` で
  値読み取りたい code (HUD / Brain target / attack inner skip) で `MarkerEnum` 的な値表現が
  別途必要になる。enum component の値読み取りやすさと開発体験を優先して採用しなかった。
- **既存 3 marker を残しつつ Side enum を追加**: 並行運用で本 Issue の動機 (継ぎ足し増殖の
  解消) を逃す。混在期の長期化で結局後でやり直し。本 Issue 範囲で完全置換する方を選んだ。
- **attack resolve を完全 1 系統に統合**: `iter_combinations_mut` で全 character pair を
  回す案。disjoint 解析は不要になるが、attacker / victim の方向判定を毎 pair で 2 回試行
  する必要があり、`LastEngagedWith` の Optional 化と animation library の分岐で複雑性が増す。
  Hero / Villain attacker の 2 経路に分けた方が `try_apply_attack_to_victim` の共通利用と
  Bevy schedule の解析しやすさが両立する。
- **BotConfig は新設せず MeleeConfig 流用**: Phase 3 の挙動をそのまま YAML 化する案。
  `ai: kind: bot` の中身が `MeleeConfig` だと「kind=bot なのに melee 系 field」という
  semantic ズレが出る。`BotConfig` を独立した data type にし、`into_melee_config()` で
  詰め替える形で「YAML スキーマは Bot 専用、内部 Brain は MeleeConfig 流用」を明示する。
- **`AiConfig::Bot` YAML 経路で env var を廃止**: env var 経路は sample-projects との
  相性 (= hero YAML を書き換えずに bot 化できる) で残す価値がある。Phase 3 補追規約を
  維持し、両立 + env 優先で実装。
- **HUD ally HP bar / KO 演出も本 Issue でやる**: scope が膨らみすぎる (ADR-0030/0031/0032/0033
  全部に Phase 補追が要る)。本 Issue は runtime 対称化 (attack resolve + knockback + KO 遷移)
  までで切り、HUD は別 Issue (ADR-0030 系の Phase 補追) で扱う。

## Consequences

**得られたもの**
- Enemy → Ally の chase / damage が runtime レベルで対称化 (= Phase 4 の主要動機の達成)
- 新 victim / 新 controller を増やしても resolve system / Brain target を増殖させずに
  済む拡張性
- character YAML の `ai: kind: bot` で hero を bot 化できる宣言的経路。env var との両立で
  既存挙動 (sample-projects/minimal の hero 手動操作) を回帰なく維持
- ADR-0035 Decision §「ランタイムすり替え対応」の予告を実装。Mind Control 系の
  (`Side` を保ちつつ `Controller` を動的切替) base 構造ができた

**支払うコスト**
- `Player(PlayerId)` / `Enemy` / `Ally` を呼ぶコードが全削除されたので、外部 (editor 等)
  からこの marker を参照していた箇所は壊れる。現状本 repo 内では engine 内のみ参照しており
  影響なし (確認済)。editor は独自スライス (CLAUDE.md FSD 規約「editor / engine は独立を維持」)
- attack resolve の disjoint を `MeleeBrain` の有無で取る規約により、「Hero に `MeleeBrain` を
  attach する将来用途」(例: Mind Control で Hero に Villain Brain を attach) が破綻する。
  そのケースは本 ADR の scope 外で、別 ADR で MeleeBrain rename or Brain trait 抽出 (Issue #7)
  と合わせて再設計する
- `BotConfig` は当面 `MeleeConfig` 同形でフィールド重複。`into_melee_config()` の薄い変換が
  挟まる。Bot 専用 param が出てきたら本格運用、それまでは [memory: dedup_later] 維持

**今後の拡張余地**
- HUD ally HP bar / overhead bar / engagement-link / icon HUD は ADR-0030/0031/0032/0033 の
  Phase 補追として別 Issue で実装。`EnemyTag` の rename (= `VillainTag` 化) は editor UI 対応
  (Issue #8) と合わせて検討
- Brain trait 抽出 + `TargetSelector` enum 化 (Issue #7) は本 ADR の `Side` filter ベースに
  そのまま乗る (= target Query を Side で取って selector で絞り込む形)
- Mind Control 系 (戦闘中の `Controller` 動的切替) は `Side` を保ちつつ `Controller` component を
  remove → add する形で実装可能。アニメ供給 (`PlayerAnimationLibrary` Resource vs entity 持ち
  `EnemyAnimationSet`) の動的切替は別 ADR

## 関連

- [ADR-0035](0035-character-ai-three-layer-fsm.md): Phase 1-3 で Brain 3 種類を導入した経緯と
  Decision §「ランタイムすり替え対応」での Side / Controller 分離予告
- [ADR-0031](0031-enemy-hp-bar-and-engagement-tracking.md): `LastEngagedWith` (Player → Enemy)
  の HUD engagement-link 起点。本 ADR で `Controller::Human` 限定の attach 条件を明示化
- [ADR-0036](0036-enemy-to-player-damage-symmetry.md): Enemy → Player の damage / knockback
  対称化。本 ADR は同じ流れを Hero side 全体に拡張 (= Enemy → Ally への対称化)
- [ADR-0030](0030-multi-player-hud-targeting.md): `PlayerId` (P1..P4)。本 ADR で
  `PlayerId` を独立 component に分離し、`Controller::Human` 持ち entity に attach する
- Issue #6 — 本 ADR の実装 Issue
- Issue #7 — Brain trait 抽出 + `TargetSelector` enum 化 (本 ADR の Side filter ベースに乗る)
- Issue #8 — editor UI 対応 (`AiConfig::Bot` の dropdown を含む)
- `packages/engine/src/features/character/movement.rs`: `Side` / `Controller` 定義、
  `player_input_controller` / `camera_follow` の filter
- `packages/engine/src/features/character/ai.rs`: 3 Brain の Side filter 切替、
  `nearest_on_side` / `nearest_hero_human` helper
- `packages/engine/src/features/character/attack.rs`: `resolve_hero_attacks` /
  `resolve_villain_attacks` (旧 3 系統 → 2 系統)
- `packages/engine/src/entities/character/model.rs`: `AiConfig::Bot(BotConfig)` variant、
  `BotConfig` struct、`into_melee_config()` 変換
- `packages/engine/src/scenes/battle.rs`: hero / enemy / ally spawn の Side / Controller
  attach、`AiConfig::Bot` の env var + YAML 両立処理
