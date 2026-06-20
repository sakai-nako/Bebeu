# ADR-0010: Undo/Redo を editor session-scope の snapshot 履歴で実装する

## Status

Accepted

## Context

SpriteGroup Editor に Undo/Redo を入れる。対象は draft（編集中の `SpriteGroup`）への変更（pivot 移動、Box の追加 / 移動 / リサイズ、Sprite[0] 一括反映、import 等）。要件:

1. **編集セッション単位で完結**: editor を閉じれば履歴は捨てる。再度開いた時はゼロから始まる
2. **保存しても履歴を消さない**: 保存後にもう一手戻したい、という UX を満たす
3. **mutation API が増えても破綻しない**: 新しい操作を追加するたびに「Undo 用の逆操作」を書く必要がない
4. **同期方針を守る**（→ ADR-0002）: async / 追加 runtime を入れない
5. **キャパシティはユーザー設定で変えられる**

draft 自体は `Signal<SpriteGroup>` で持っている（widgets/character/ui/README.md 参照）。Undo/Redo はこの値をどう巻き戻すか、という話に帰着する。

## Decision

3 つの設計判断をセットで採用する。

### 1. Snapshot ベース（diff / command ではない）

`History<T>` は **`T` のスナップショット** をスタックに積む。draft 全体を `clone()` する。

```rust
pub fn record(&mut self) {
    let snapshot = self.target.peek().clone();
    self.history.write().record(snapshot);
}
```

実装は `shared/history.rs`。`record()` は **mutation 直前**に呼び、`undo()` は past から 1 件 pop して `target.set(prev)` する（現在値は future に積んで redo 可能にする）。

### 2. Session-scope（editor mount で生成、unmount で消える）

`use_history(draft, capacity)` を `SpriteGroupEditor` の中で `use_signal` で初期化する。Dioxus の Signal は component と寿命が一致するので、editor を抜ければ history も自動で破棄される。Save 時には何もしない（history はそのまま）。

### 3. Capacity は preferences から `peek()` で凍結

```rust
let preferences = use_preferences();
let history_capacity = preferences.peek().sprite_group_history_capacity as usize;
let history = use_history(draft, history_capacity);
```

`peek()` で読むので reactive subscription を張らない。editor を開いている間、preferences が変わっても capacity は固定。次に editor を開いたタイミングで反映される。

## Alternatives Considered

- **Diff / Command パターン（逆操作を保持）**: 操作ごとに `apply` / `revert` を書く必要がある。mutation を 1 つ増やすたびにペアの実装と整合性テストが要る。snapshot なら `record()` を 1 行入れるだけ。SpriteGroup は数 KB オーダー / 履歴上限 50 程度なので clone のメモリコストは無視できる。

- **永続化する履歴**: 「未保存の編集の履歴」と「保存済み状態への復帰」が混ざり、Save / Load のセマンティクスが破綻する（保存後に Undo すると disk と画面が乖離するなど）。session-scope に閉じれば「画面を閉じたら履歴も終わり」という素直なルールになる。

- **Save 後に history をクリア**: 「保存したけどやっぱり戻したい」を奪うので UX が悪い。一方で、保存後に Undo すると disk と乖離するが、再 Save すれば追従するので実害なし。

- **Capacity を reactive にする**: editor を開いている最中に capacity を増減した時に history を作り直す＝既存の past が消える。直感に反するし、preferences 変更が「次回起動から反映」で十分なケース（キャパは滅多に変えない）なので、初回 init で凍結する方が予測可能性が高い。

- **Preferences を経由せずグローバル定数で固定**: ユーザーが性能と履歴の深さのトレードオフを選べない（マシンスペックや作業の重さで好みは分かれる）。

- **`UseHistory` を非 Copy（`&mut` で受ける）**: editor の onclick / use_effect / use_keyboard_action 等に同じ history を配る必要がある。Copy にしておけばクロージャごとに別コピーを掴めるので props 配線が単純になる（中身は `Signal<History<T>>` + `Signal<T>` の組なので Copy で問題ない）。

## Consequences

**得られたもの**

- 新しい mutation を追加するときの規約は **「draft を変える前に `history.record()` を 1 行入れる」だけ**。`features/character/ui/apply_first_sprite_to_others.rs` / `widgets/character/ui/sprite_canvas.rs`（mousedown）/ `widgets/character/ui/sprite_property_panel.rs`（onchange）/ `features/character/ui/edit_sprite_group.rs`（Add Box / pivot 移動）が全部この規約で書かれている。
- editor を閉じれば履歴は自動的に破棄される。`use_drop` 等のクリーンアップは要らない。
- capacity 0 を防ぐため `History::new(capacity.max(1))` で底打ちしている。仮に preferences が壊れても「常に 1 件は持てる」。

**支払うコスト**

- `record()` を **mutation 直前** に呼ぶ規約を人間が守る必要がある。後から呼ぶと「変更後の状態」を past に積んでしまい Undo が空回りする。違反を防ぐリンタはない（`#[must_use]` も付かない）のでレビューと PR テンプレで担保。
- snapshot の clone コストは draft サイズに比例する。SpriteGroup（pivot / HitBox 配列）程度なら問題ないが、巨大な集約を後付けで対象にするときは要再評価。
- 同値の連続 record を防ぐため、呼び出し側でガードが必要なケースがある（`sprite_property_panel.rs` の数値入力は「値が同じなら return」で record をスキップ）。

**今後の拡張余地**

- 「複数 editor 同時に開く」になったら、history は editor インスタンス単位で持っているので衝突しない（global state を共有していないため）。
- Animation Editor など別の editor が増えた時は、それぞれの draft Signal に `use_history` を生やせば同じ規約で動く。capacity は editor 種別ごとに preferences フィールドを増やす（`animation_history_capacity` 等）。
- snapshot コストが問題になる規模に成長したら `History<T>` を `History<Diff<T>>` にする換装が容易。`UseHistory` の facade は変えずに済む。
