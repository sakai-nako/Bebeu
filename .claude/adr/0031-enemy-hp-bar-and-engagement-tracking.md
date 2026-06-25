# ADR-0031: Enemy HP bar の動的 target 解決と HUD 要素間 anchor

## Status

Accepted (2026-06-25 に Phase 2 として `enemy_hp_bar` kind、`EnemyTarget` enum、`anchor_to` field、
engagement tracking、`Character.tag` を一括導入)

## Context

ADR-0030 で Player HP HUD に `target: PlayerId` を入れ、HUD 要素が「どの Player を映すか」を
持つようにした。次に「Enemy の HP も HUD に出したい」という需要に対応する必要が出た。
具体的に欲しい用途は以下の 3 つ:

1. **engagement-link**: 「P1 の HP bar の隣に、P1 が直近で殴った enemy の HP bar を出す」。
   4 人 co-op で各自の「今戦ってる相手」を視認させる用途。
2. **boss bottom-fixed**: 画面下部に固定で「ボスの HP bar」を出す。Enemy YAML の tag で boss
   を識別し、その entity の HP を映す。
3. **頭上追従** (Phase 3 で扱う): Enemy entity の頭上に world-space で HP bar を表示する。

(1) と (3) は target が時間とともに変わる (engagement の対象が変わる / Enemy が増減する) 動的
解決が必要で、(2) は静的解決で済む。さらに (1) は「他の HUD 要素 (P1 HP bar) の隣に置きたい」
ため、HUD 要素同士で位置関係を持つ仕組みが要る。

(2) と (3) を分けるべきかは前回の検討で「描画機構が違う (screen 子 / world 子) 軸でだけ kind を
分ける」とした。Phase 2 では (1) と (2) (= 全 screen-anchored 系) を `enemy_hp_bar` の **1 kind**
にまとめ、(3) は Phase 3 で別 kind `enemy_overhead_hp_bar` として扱う。

## Decision

### 4 つの拡張を同時に入れる

| 拡張 | 対象 | 役割 |
|------|------|------|
| `Character.tag: Option<String>` | YAML schema (engine + editor) | boss 識別ラベル。Enemy entity の `EnemyTag` component に乗る。 |
| `Player.LastEngagedWith: Option<Entity>` | engine ECS | resolve_hits で hit が決まった瞬間に書き込み。engagement-link の resolver が引く。 |
| `HudElement.id` / `anchor_to: HudElementAnchor` | YAML schema | HUD 要素間の位置関係。`id` を持つ要素を、後続要素が `anchor_to: { id, edge }` で参照する。 |
| `HudElement::EnemyHpBar(EnemyHpBarConfig)` + `EnemyTarget` enum | YAML schema | enemy HP bar の表現。target は `LastEngagedBy` / `Tag` / `NthEnemy` の 3 種。 |

### `EnemyTarget` の 3 variant

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyTarget {
    LastEngagedBy(PlayerId),  // YAML: { last_engaged_by: p1 }
    Tag(String),              // YAML: { tag: boss }
    NthEnemy(usize),          // YAML: { nth_enemy: 0 }
}
```

externally-tagged enum で 1 key = 1 variant の形に揃え、boss 用途 / debug 用途 / engagement-link
を 1 enum で扱えるようにする。default は `LastEngagedBy(P1)` (co-op の最頻ケース)。

将来 Player target (HUD 全 kind 共通) と統合する案は、kind ごとに target 型が違う方が型安全
なので採用しない。HudElement::PlayerHpBar.target は `PlayerId`、HudElement::EnemyHpBar.target は
`EnemyTarget` で別系統。

### `anchor_to` で HUD 要素間の参照

`HudElement` の各 Config に `anchor: HudAnchor`, `anchor_to: Option<HudElementAnchor>`, `offset: HudOffset`
の 3 field がある。`anchor_to.is_some()` のとき `anchor` は無視され、参照先要素の `edge` から
`offset` ぶんずらした位置に置かれる:

```yaml
- kind: player_hp_bar
  id: p1_hp                            # 他要素から参照される識別子
  target: p1
  anchor: top_left
  offset: { x: 16.0, "y": 16.0 }
  size: { w: 120.0, h: 8.0 }
- kind: enemy_hp_bar
  target: { last_engaged_by: p1 }      # P1 が直近で殴った enemy
  anchor_to:                            # P1 HP bar の bottom-left を基準
    id: p1_hp
    edge: bottom_left
  offset: { x: 0.0, "y": 4.0 }
  size: { w: 120.0, h: 6.0 }
```

実装は Bevy の Transform hierarchy をそのまま使う:
- `anchor_to.is_none()` → 要素は `MainCamera` の子 (= screen-anchored、camera 追従)
- `anchor_to.is_some()` → 参照先要素の root の子 (= 親が camera に追従していれば連鎖で追従)

参照は **前方向 (= YAML 内で親が先に書かれている)** のみ。spawn loop が `HashMap<id, (entity, size)>` を
順次積みながら resolve する。未解決 id は warn 出して要素 skip、循環参照は構造的に不可能。

### Enemy HP bar は単一 gauge 固定 (Phase 2)

target が時間で変わる (LastEngagedBy では engagement 切替で別 enemy になる) ため、複数 gauge
segment を持たせると「target 切替時に max_hp が変わり segment 範囲が壊れる」問題が出る。
Phase 2 では **EnemyHpBar は常に単一 gauge** で実装し、`gauge_step` field は schema 互換性の
ため残すが engine 側は無視する。multi-gauge enemy bar (Dark Souls 系のボス) は Phase 2.5 / 3
で「target 切替時に root を despawn + rebuild」する形で別途扱う。

### resolve_hits で LastEngagedWith を書く

```rust
// packages/engine/src/features/character/attack.rs
HitDecision::BreakAttack => {
    last_engaged.0 = Some(enemy_entity);
    break;
}
```

Player の Query に `&mut LastEngagedWith` を追加し、1 attack 1 hit が成立した瞬間に書き換える。
Enemy → Player の被弾 (Phase B) はまだ未実装なので、Player → Enemy 方向の hit でしか書かれない。

### per-frame target 解決と Visibility

`EnemyHpBarRoot` component に target を持たせ、`update_enemy_hp_bar` system が毎 frame:

1. target を resolve → Some(entity, hp) or None
2. Some なら `Visibility::Inherited`、None なら `Visibility::Hidden`
3. gauge sprite を current/max の ratio で縮める

Visibility は Bevy の hierarchy で子全部に伝播するので、target 不在のときは frame / bg / gauge
全部が一度に消える。

## Alternatives Considered

- **Element anchor を `HudAnchor` enum に untagged で混ぜる**: `anchor: top_left` (string) と
  `anchor: { id, edge }` (struct) を 1 field で受ける案。シンプルだが既存の YAML schema 移行と
  editor UI の picker 切替が同時に要り、Phase 2 のスコープが膨らむ。`anchor_to` を別 field に
  することで「anchor は変わらず、参照モードを後付け」できる。
- **EnemyHpBar の target を Player target と統合した `HudTarget` enum**: kind ごとに target の
  意味が違うので、HudElement::target() の戻り値を `HudTarget` enum にすると downcast が要る。
  kind 単位で target 型を分けたほうが pattern match で素直。
- **EnemyHpBar の multi-gauge を Phase 2 で実装**: target 切替時の rebuild が要る。Phase 2 の
  スコープを膨らませる割に、欲しい場面 (boss HP) は Tag target (静的) で扱えるので Phase 2 で
  multi-gauge 制約を入れても実害は薄い。
- **LastEngagedWith を Enemy 側に置く (`Enemy.LastAttackedBy: Option<PlayerId>`)**: HUD 側は
  enemy_query を全走査して `last_attacked_by == p1` な enemy を探す。Player 側に持つほうが
  「Player ごとに 1 つの engagement state」が直感的で、resolver も O(1)。多重 hit の場合は
  時間順で最新が勝つ意味論を維持するため。
- **`Tag` を string ではなく enum (TagId)**: 型安全だが新キャラ追加で enum 拡張が要る。
  string なら character YAML 編集だけで完結。HUD 側で typo は warn 出して skip。

## Consequences

**得られたもの**

- Engagement-link の動的 enemy bar が `target: { last_engaged_by: p1 } + anchor_to: { id: p1_hp,
  edge: bottom_left }` の数行 YAML で書ける。
- Boss bar は `target: { tag: boss } + anchor: bottom` で 1 要素として表現できる。Character YAML
  に `tag: boss` を足すだけ。
- HUD 要素間の位置関係は Bevy Transform hierarchy で勝手に追従する (camera follow / 相対位置)。
- `Player.LastEngagedWith` は engagement-link 以外の用途 (例: damage source 表示) にも転用可能。
- `Character.tag` は Phase 3 の overhead bar や、debug overlay の boss 判定にも使い回せる。

**支払うコスト**

- `HudElement` から `Copy` を外したことで、UI 側で `*element` だった箇所が `element.clone()` に。
  cost は無視できる小さい構造体で、明示的 clone のほうが意図が読みやすい。
- HUD 要素間の id 参照は前方向のみ。後方参照 (= 親が後に書かれている) は warn + skip で
  容認しているが、editor の lint で事前検知するのは future work。
- EnemyHpBar は単一 gauge 固定で multi-gauge は使えない (gauge_step / gauge_gap を YAML に
  書いても engine が無視)。boss 多段 HP のような表現は Phase 2.5 / 3 で別途。
- attack.rs の Player Query に `&mut LastEngagedWith` が増え、bundle tuple 上限に近づく
  (現状は余裕あり)。Phase 2 までは bevy の query は 14 element までで OK。

**今後の拡張余地 (Phase 3 / 後続)**

- `enemy_overhead_hp_bar` kind (world-anchored) を Phase 3 で追加。target は同じ EnemyTarget
  enum を流用。spawn は `Added<Enemy>` で per-enemy。
- HudElement `target` accessor を `HudTarget` enum 化するか、Phase 3 で再評価。
- Multi-gauge enemy bar (boss 多段 HP) は target 切替時の rebuild 機構と一緒に Phase 2.5 で。
- engagement timeout (5 秒触ってない enemy は HUD から消える) は `LastEngagedWith` に
  `frames_since_engaged` を足す形で後付け可能。
- Element anchor の **editor preview** (UI 上で「P1 HP bar の bottom-left に EnemyHpBar が
  くる」を視覚的に示す) は dragable preview UI が要るので別 ADR で。

## 関連

- [ADR-0029](0029-hud-layout-in-project-yaml.md): HUD レイアウトを Project YAML に持つ方針。
  `enemy_hp_bar` は同じ internally-tagged enum の variant として追加した。
- [ADR-0030](0030-multi-player-hud-targeting.md): PlayerId と Player HUD の target スキーマ。
  本 ADR の Player 側 target はこれを再利用、Enemy 側は別 enum (`EnemyTarget`)。
- [ADR-0024](0024-knockback-gauge-attackboxmeta-driven-hit.md): 攻撃解決の流れ。
  `LastEngagedWith` の書き込みは hit 成立点 (`HitDecision::BreakAttack`) に乗せた。
- `packages/engine/src/entities/project/model.rs`: `EnemyHpBarConfig` / `EnemyTarget` /
  `HudElementAnchor`
- `packages/engine/src/features/hud.rs`: spawn loop / Element anchor resolve /
  `update_enemy_hp_bar`
- `packages/engine/src/features/character/movement.rs`: `EnemyTag` / `LastEngagedWith`
- `packages/engine/src/features/character/attack.rs`: `LastEngagedWith` の書き込み点
- `packages/engine/src/entities/character/model.rs`: `Character.tag`
- `packages/editor-desktop/src/features/hud/ui/edit_hud_layout.rs`: `EnemyHpBarEditor`
