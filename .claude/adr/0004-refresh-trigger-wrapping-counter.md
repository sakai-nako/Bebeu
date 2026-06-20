# ADR-0004: Refresh trigger as wrapping `u64` counter

## Status

Accepted

## Context

mutation（Create / Delete 等）後に一覧 view を再フェッチさせる仕組みが必要。要件:

1. mutation 側は一覧 view の存在 / 場所を知らずに発火できる（layer isolation を壊さない）
2. 同フレーム内で複数の bump が起きても各 bump が確実に subscriber に届く（取りこぼさない）
3. Dioxus の sync な reactivity だけで完結する

要件 2 が嫌らしい。Dioxus の `Signal` は **同じ値を set しても reactivity を発火しない**（メモ化されている）。bool toggle で実装すると、同フレームで 2 回トグルされた時に値が元に戻り「変化しなかった」と判定されることがある。

## Decision

`Signal<u64>` を newtype `CharactersRefreshTrigger` で包み、`.bump()` は `wrapping_add(1)` で値を進める。subscriber は `use_effect` 内で `.subscribe()` を呼んで値を読む（Dioxus は read を tracking する）。

```rust
pub struct CharactersRefreshTrigger(Signal<u64>);

impl CharactersRefreshTrigger {
    pub fn subscribe(&self) -> u64 { self.0.read().to_owned() }
    pub fn bump(&mut self) {
        let next = self.0.read().wrapping_add(1);
        self.0.set(next);
    }
}
```

実装は `entities/character/refresh.rs`。

## Alternatives Considered

- **`Signal<bool>` toggle**: 同フレームで 2 回トグルすると元の値に戻り、変化検知が漏れる。
- **`tokio::sync::broadcast` / channel**: editor は同期方針（→ ADR-0002）なので runtime 依存を増やしたくない。
- **mutation 側が一覧の `Signal<Vec<T>>` を直接書き換える**: features → widgets の参照が必要になり layer isolation を壊す。
- **bool + epoch number の組合せ**: counter 単独で要件を満たすので不要に複雑。

## Consequences

- `bump()` は安価で副作用のないリトライが可能（複数回 bump しても render 側は 1 回 effect 再実行）。
- `u64::MAX` で wrap するが、人間操作の頻度では事実上発生しない。
- 「何を refresh するか」の粒度はトリガー単位。今は `CharactersRefreshTrigger` 1 種だけ。entity ごとに分割したくなったら `{Entity}RefreshTrigger` を追加する。
- 一覧丸ごと再フェッチなので、entity 単位の差分更新は将来の課題。
