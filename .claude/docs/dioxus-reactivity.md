# Dioxus 0.7 リアクティビティ・ベストプラクティス

Dioxus 0.7 のリアクティビティは Signal を中心とした購読モデルで成り立っている。本ドキュメントでは「**いつ Signal 型を使うか**」「**いつ素の値で良いか**」「`use_effect` が再実行されない時の対処」を整理する。

---

## 基本ルール: 「**変化する値はすべて Signal/ReadSignal 型として表現する**」

| データの種類 | 推奨される型 |
|---|---|
| 自分のコンポーネントが持つ可変状態 | `Signal<T>` (`use_signal(...)`) |
| **親から渡される可変 prop**（ルートパラメータ等） | `ReadSignal<T>` |
| 親から渡される実質固定の prop | `T` |
| グローバル/コンテキスト共有 | `Signal<T>` (`use_context_provider`) |
| 派生値（他 Signal の関数として計算されるもの） | `Memo<T>` (`use_memo(...)`) |

> **`ReadOnlySignal<T>` は 0.8 で削除予定**。Dioxus 0.7.6 以降は `ReadSignal<T>` を使う。`use dioxus::prelude::ReadSignal;` でインポートできる。

---

## Prop が変わるのに `use_effect` が再実行されない問題

### アンチパターン

```rust
#[component]
fn UserPage(id: String) -> Element {
    use_effect(move || {
        match repo.get(&id) {  // id はクロージャ作成時のスナップショット
            Ok(_) => { /* ... */ }
            Err(_) => { /* ... */ }
        }
    });
}
```

`id: String` は **値型 prop**。クロージャは `move` で `id` を 1 度キャプチャするだけで、ルート遷移などで `id` が変わっても `use_effect` は再実行されない（`use_effect` は **読まれた Signal の変化** にしか反応しない）。

### 解決策 A（推奨）: prop を `ReadSignal<T>` にする

```rust
#[component]
fn UserPage(id: ReadSignal<String>) -> Element {
    use_effect(move || {
        let _ = id();  // 関数呼び出し () で読むと購読が成立する
        match repo.get(&id()) {
            Ok(_) => { /* ... */ }
            Err(_) => { /* ... */ }
        }
    });
}
```

- 親側のコード変更は不要（`String` から `ReadSignal<String>` への自動変換が効く）
- ルーターから渡されるパラメータも `String` のまま `#[route]` 定義可能（`Routes::UserPage { id: String }` のままで OK）
- 同じコンポーネントで再レンダリングされるたびに ReadSignal の値が更新され、effect が反応する

### 解決策 B（特殊用途）: `use_reactive` で明示的に依存追跡

```rust
use_effect(use_reactive((&id,), move |(id,)| {
    match repo.get(&id) { /* ... */ }
}));
```

- prop の型を変えられない場合（外部 crate との境界等）の出口
- タプル構文 `((&x,), |(x,)| ...)` がやや独特、単要素タプルの末尾 `,` 必須
- 通常は **解決策 A を選ぶ**

---

## `Signal<T>` と `ReadSignal<T>` の違い

| 観点 | `Signal<T>` | `ReadSignal<T>` |
|---|---|---|
| 読み取り | `signal()` または `signal.read()` | 同じ |
| 書き込み | `signal.set(...)`, `signal.write()` | 不可（read-only） |
| 主な用途 | 自身の状態 / 共有可変状態 | 親→子 への可変 prop |
| Copy | ◯（参照カウントのコピー） | ◯ |

**Prop に書き込む必要はないが、購読したい** → `ReadSignal<T>`
**自分で生成して内部だけで mutate** → `Signal<T>` (`use_signal`)

---

## `Signal` の値が変わったかの判定

`Signal::set` は内部で `PartialEq` で比較し、値が変わっていなければ購読者へ通知しない場合がある。**「同じ値を再代入してリフレッシュを誘発する」のは不確実**。

### 確実なリフレッシュ誘発: カウンターパターン

```rust
#[derive(Clone, Copy)]
pub struct RefreshTrigger(Signal<u64>);

impl RefreshTrigger {
    pub fn subscribe(&self) -> u64 { self.0.read().to_owned() }
    pub fn bump(&mut self) {
        let next = self.0.read().wrapping_add(1);
        self.0.set(next);
    }
}
```

bump のたびに必ず値が増分するので、購読者の `use_effect` が確実に再実行される。bool トグルや「同値再代入」より堅い。

> 本プロジェクトでは [`entities/character/refresh.rs`](../../packages/editor/src/entities/character/refresh.rs) の `CharactersRefreshTrigger` で採用。

---

## チェックリスト

新しい `#[component]` を書く時:

1. **Prop は変化しうるか？**
   - Yes → `ReadSignal<T>` または `Signal<T>` で受ける
   - No  → 素の `T`
2. **`use_effect` の中で、依存にすべき値はすべて Signal 経由で読んでいるか？**
   - Yes → OK
   - No  → 該当値を Signal 化、または `use_reactive` でラップ
3. **`Signal::set` で同値書き込みに頼っていないか？**
   - 値変化を確実にしたい場合はカウンター/版番でリフレッシュ
4. **`ReadOnlySignal` を書いていないか？**
   - 0.7 以降は `ReadSignal` に置き換え
