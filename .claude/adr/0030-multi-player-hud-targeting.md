# ADR-0030: HUD 要素は Player を target で指定する

## Status

Accepted (2026-06-25 に Phase 1 として PlayerId と HUD `target` field を導入)

## Context

ADR-0029 で導入した `PlayerHpBar` / `PlayerHpRing` は暗黙に「the Player」を映していた。
engine 側でも `Player` は zero-sized marker で個体を区別できず、`HUD update` system は
`player_query.single()` で 1 体前提のクエリを掛けていた。

将来 local co-op (最大 4 人) を入れる前提で、

- 「P1 / P2 のどの HP を見せるか」を HUD 要素ごとに指定したい
- 同じ `PlayerHpBar` kind を 2 つ並べて P1 と P2 用に anchor を変えたい
- engine 側でも Player を id で識別したい (将来の入力ソース紐付け、被弾解決の起点付け)

の 3 つを同時に満たす必要がある。

これらは **Phase 1 で固める**: 「target スキーマ」と「Player の id 化」の 2 つだけ。
engagement-link 系の Enemy HP bar (ADR-0030 後継で扱う予定) や input-source 紐付けは
別 ADR で追う。

## Decision

### `PlayerId` を `shared` 層に定義する

```rust
// packages/engine/src/shared/player_id.rs
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerId {
    #[default] P1, P2, P3, P4,
}
```

- YAML key は `p1` / `p2` / `p3` / `p4`。default は `p1`。
- **shared 層** に置く理由: `entities/project` (HUD config) と `features/character`
  (ECS marker) の両方から参照される横断データ型で、どちらの slice にも属さない。
  ADR-0001 FSD で entities が features に依存する形は禁止しているため、共通祖先である
  shared に置くのが正規。
- 上限 4 は beat-em-up local co-op の実用上限 (Streets of Rage / Final Fight / Castle
  Crashers 系)。それ以上が要れば後で variant を増やす。`u8` newtype ではなく enum に
  したのは、`serde(rename_all)` で YAML key を素直に書ける + `PlayerId::ALL` で列挙できる
  からで、type safety を優先した。

### `Player` marker を `Player(PlayerId)` に昇格

```rust
// packages/engine/src/features/character/movement.rs
#[derive(Component, Debug, Clone, Copy)]
pub struct Player(pub PlayerId);
```

既存の `With<Player>` filter はそのまま動く (tuple struct でも Bevy の filter は OK)。
id が必要な system は `Query<&Player>` で `player.0` を読む。

construction 側 (`scenes/battle.rs`) は当面 `Player(PlayerId::P1)` 固定。`project.players`
が複数あるときに `P1..P4` を順番に割り振るのは Phase 2 以降。

### HUD 要素に共通 `target: PlayerId` field

`PlayerHpBarConfig` / `PlayerHpRingConfig` の両方に `#[serde(default)] target: PlayerId`
を生やし、`HudElement::target()` accessor を `anchor()` / `offset()` と同じ位置に追加した。

```yaml
hud:
  elements:
    - kind: player_hp_bar
      target: p1                # 省略時は p1
      anchor: top_left
      offset: { x: 16.0, "y": 16.0 }
      size:   { w: 120.0, h: 8.0 }
    - kind: player_hp_bar       # 同じ kind を 2 個並べる
      target: p2
      anchor: top_right
      offset: { x: -16.0, "y": 16.0 }
      size:   { w: 120.0, h: 8.0 }
      fill_direction: right_to_left
```

HUD update system は `Query<(&Player, &HitPoints)>` から `gauge.target` 一致の Player を
探して current/max HP を引く。**該当 Player が居ない要素は spawn 時点で warn 1 回出して
skip** する (P2 用 HUD を P2 不在 project で出さない)。fail-hard にしないのは「project に
共通 HUD layout を持たせて player 数だけ変えたい」需要を見越したため。

### `target` を kind 横断の共通 field にする (variant ごとには持たない)

`HudElement` は internally-tagged enum で、serde の制約上 enum レベルでは共通 field を
表現できない。よって `target` は各 Config struct に持たせるが、**意味的には全 variant で
共通**として扱い、`HudElement::target()` で一括取得する。

将来の `EnemyHpBar` variant (engagement-link 用、ADR-0030 後継で議論) では `target` の
意味が「どの Player を映すか」から「どの Enemy を映すか」に変わる。そのため
`HudElement::target()` accessor は **Player 系 variant 限定** のシグネチャ
(`fn target(&self) -> PlayerId`) ではなく、後に `fn player_target(&self) -> Option<PlayerId>`
に分けるか、`HudTarget` enum に置き換える形を予約する。Phase 1 では Player variant しか
無いので前者で十分。

## Alternatives Considered

- **PlayerId を `features/character` に置く**: ADR-0001 の FSD で entities は features に
  依存できないため不可。HUD config (entities) から見えないと target を表現できない。
- **`Player` marker と `PlayerId` component を分ける**: `With<Player>` filter は変わらず、
  id 取得時に追加で `&PlayerId` を query する形。tuple struct より「entity に Player 性と
  id が同期して付く」保証が弱い (片方だけ attach するバグの余地)。1 component で済む
  `Player(PlayerId)` のほうが安全。
- **target を YAML で省略可能にせず必須にする**: 既存の project YAML が壊れる。Phase 1
  は backwards-compat で省略時 P1 を返す `#[serde(default)]` で済ませ、`target: p1` を
  明示するかは editor / project 作成者の裁量に任せる。
- **`HudTarget` enum (`Player(PlayerId)` / `Enemy(EnemyTarget)`) を最初から導入する**:
  Enemy variant は Phase 2 で導入する予定なので、enum 化は Phase 2 でやる方が「実需に
  合わせて作る」原則に沿う。Phase 1 で先取りすると Enemy 側の不確定要素 (engagement /
  tag / nth_enemy の選択肢) が enum の形を引きずる。

## Consequences

**得られたもの**

- HUD 要素を `target: p2` で重複 spawn できるようになった (multi-player を見越した最小準備)。
- `Player(PlayerId)` で engine 側も id を持つ entity になり、将来の input source 紐付け
  (`HashMap<PlayerId, ActionMap>`) や engagement tracking (Phase 2 の `LastEngagedBy`) の
  起点ができた。
- HUD update system が「該当 Player 不在の要素を skip」する形になり、project YAML を変えず
  に Player 数を増減できる土台ができた。

**支払うコスト**

- editor / engine の両方に `PlayerId` を mirror する必要がある (ADR-0001 の独立性方針通り
  だが、struct 同期コストは継続的に発生)。
- `HudElement::target()` は Player variant 専用シグネチャで、Enemy variant が入る Phase 2
  でリネーム or `HudTarget` enum 化のリファクタが入る (この移行は Phase 2 ADR で計画する)。

**今後の拡張余地 (Phase 2 以降の予約)**

- `battle.rs` の player spawn を `project.players` 順に `P1..P4` で割り振る。
- `ActionMap` を Player ごとに分離 (`HashMap<PlayerId, ActionMap>`)。
- `EnemyHpBar` variant の追加と `HudTarget` enum 化 (engagement-link / boss tag / nth_enemy)。
- HUD 要素間の親子参照 (`id` + `Element` anchor)。Phase 2 で engagement-link 描画と同時に
  入る。

## 関連

- [ADR-0001](0001-adopt-feature-sliced-design.md): FSD 層規約。`shared::player_id` は
  この方針に従って entities / features の共通祖先に置いている。
- [ADR-0029](0029-hud-layout-in-project-yaml.md): HUD レイアウトを Project YAML に持つ
  方針。本 ADR はそのスキーマに `target` field を追加する後継。
- `packages/engine/src/shared/player_id.rs`: PlayerId 定義
- `packages/engine/src/features/character/movement.rs`: `Player(PlayerId)`
- `packages/engine/src/features/hud.rs`: target で filter する spawn / update system
- `packages/engine/src/entities/project/model.rs`: HUD config の target field
- `packages/editor-desktop/src/entities/project/model.rs`: editor 側の mirror
