# ADR-0014: Frame override の 3-state encoding (`Option<Vec<HitBox>>`)

## Status

Accepted

## Context

`Animation.frames[].body_box_overrides` / `attack_box_overrides` で、Frame 単位で Sprite の HitBox を上書きできるようにしたい。要件として **3 つの異なるセマンティクス**が必要になった:

1. **Inherit (上書きしない)**: その Frame は Sprite が定義する body/attack box をそのまま使う。普段はこれ
2. **Override (上書きする)**: Frame で独自の box リストを定義し、Sprite の box を置き換える
3. **Disable (上書きする / Sprite の box を無効化)**: その Frame は box を持たない (= 当たり判定 / 攻撃判定が無い)。回避フレーム、被弾後の無敵、攻撃の硬直などで多用する

問題は、これらをデータモデルにどう載せるか。`Vec<HitBox>` だけだと「なし」と「空 Vec」の区別ができない。型を増やすと runtime / serializer 双方の取り回しが重くなる。

## Decision

**`Option<Vec<HitBox>>` の 3 つの状態 (`None` / `Some([])` / `Some([..])`) を 3 つのセマンティクスにそのまま割り当てる。**

```rust
pub struct Frame {
    pub body_box_overrides: Option<Vec<HitBox>>,
    pub attack_box_overrides: Option<Vec<HitBox>>,
    // ...
}
```

| 状態 | データ表現 | YAML | セマンティクス |
|---|---|---|---|
| Inherit | `None` | `null` | Sprite の box をそのまま使う |
| Override | `Some([HitBox, ...])` | `[ {top_left: ...}, ... ]` | Frame で box を上書き |
| Disable | `Some(vec![])` | `[]` | Sprite の box を無効化 (box 無し) |

Editor 側 (`overrides.rs::BoxOverrideMode`) ではこの 3 状態をラジオボタン 3 択で切り替える。Override に切り替えたとき既存値が `None` または `Some([])` なら 16×16 のデフォルト box を 1 個自動追加する (`Some([])` のまま放置すると Override 状態を表現できなくなり Disable と区別不能になるため)。

Runtime (将来のゲームエンジン側) もこの 3 状態を同じ意味で解釈する規約とする。

## Alternatives Considered

- **専用 enum を作る** (`enum BoxOverrideMode { Inherit, Override(Vec<HitBox>), Disable }`):
    - 型レベルでセマンティクスを明示できる利点はあるが、`Vec` の長さで状態が決まる方の表現でも実害はない
    - serde 側で tagged enum を使うと YAML が冗長になる (`{ kind: Override, boxes: [...] }`)。untagged にすると `null` / `[]` / `[...]` で区別する形になり結局同じ
    - Sprite の `body_boxes` / `attack_boxes` も `Option<Vec<HitBox>>` で正規化されている (空のときは None) ので、Frame override 側もこの構造に揃えるほうが API の一貫性が取れる

- **bool フィールド + `Option`** (`disable: bool` + `boxes: Option<Vec<HitBox>>`):
    - `disable=true` のとき `boxes` をどう扱うか曖昧 (None? Some([])?)。整合性のための valid state チェックが要る
    - フィールドを 2 つに分ける割に表現できる状態は同じ 3 種

- **`None` / `Some` の 2 状態のみで Disable は runtime 側に任せる** (`Some([])` を Disable と解釈しない):
    - 「box 無し」を Frame 側で明示できないので、毎フレーム runtime 側に判定ロジックを持つ必要がある
    - 「Inherit したくないが box は無い」は表現できなくなり、編集 UI でも 2 択しか提供できない
    - 攻撃の硬直 / 無敵フレームを「sprite に box を置かない」で表現すると、その sprite を別の文脈 (静止ポーズ等) で使い回せない

- **`Vec<HitBox>` 単独 (デフォルトは空 Vec)**:
    - `None` 相当が表現できない。常に「Override」扱いになり Inherit が失われる

## Consequences

**得られたもの**

- データ型が `Option<Vec<HitBox>>` 1 つで済む。Sprite 側の `body_boxes` / `attack_boxes` と同じ型なので、両者で同じヘルパーを書きやすい (実際に `Frame::override_boxes_mut` / `replace_override_box` は `Sprite::boxes_mut` / `replace_box` と対称な API になっている)
- YAML round-trip が `null` / `[]` / `[..]` の 3 形態で素直に区別できる (`serde_saphyr` のデフォルト挙動)
- 既存 YAML (`body_box_overrides: null`) は touch せずそのまま動く。後付けで導入しても破壊的変更にならない
- Runtime 側は `Option<Vec<HitBox>>` を直接 match して 3 状態に分岐すれば良く、専用型のインポート / 変換が不要

**支払うコスト / 注意点**

- 「`Some([])` は Disable」という規約を runtime / editor 双方が同意する必要がある。テスト / コメント / この ADR で残しておかないと、将来の実装者が「空 Vec も None と同じ Inherit でいいよね？」と短絡する余地がある
- Editor の状態遷移ロジック (`BoxOverrideSection::on_mode_change`) で「Override に切り替える際、空のままだと Disable と区別不能になる」を毎回意識して default box を補充する必要がある。違反するとモードが意図せず Disable に化ける
- YAML を手で書くユーザーは `null` / `[]` / `[item]` を意識的に使い分ける必要がある (UI で書けば自動でこの規約に従うが、手書きする場合は注意)

**今後の拡張余地**

- 「Override + 個別 box の disable 切替」のように box 単位の細かい制御を入れたい場合は、`HitBox` 自体に `enabled: bool` のようなフィールドを生やす方向で拡張できる。3 状態の Frame レベル制御はそのまま維持
- 同じ 3-state パターンを他の override (`pivot_point_offset` 等) にも適用したくなる場合があるが、`pivot_point_offset` は `[i32; 2]` で `[0, 0]` を no-op として扱える (= 2 状態で十分) ので、現状は `Option<[i32; 2]>` のみ
