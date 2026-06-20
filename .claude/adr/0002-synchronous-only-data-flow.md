# ADR-0002: Synchronous-only data flow (no async/.await)

## Status

Accepted

## Context

Dioxus desktop の async runtime は **シングルスレッド**。`spawn(async { ... })` の中で同期 I/O をブロックすると UI スレッドごと止まる。にもかかわらず Rust の慣習では「I/O は async」が一般的で、Repository / event handler / `use_effect` のすべてに `async` を付けたくなる引力が強い。

実際にやると 2 種類の問題が起きる:

1. 中身は同期 I/O なのに見た目だけ async（"fake async"）。`async` を書く意味がないどころか、`spawn` を介して間接化されるので追跡が難しくなる。
2. ちゃんと async にすると `tokio::task::spawn_blocking` への依存が増え、「なぜ I/O だけ blocking pool に逃すのか」を説明し続ける必要がある。

## Decision

editor のコードは `async fn` / `.await` を書かない:

- Repository trait のメソッドは同期（`fn list(&self) -> Result<Vec<T>>`）
- event handler / `use_effect` / すべて同期
- 重い I/O や計算は `std::thread::spawn` で別 OS スレッドへオフロードし、結果は `Signal`（`Send`）経由で書き戻す

詳細は `.claude/docs/data-flow.md` の「async ではなく `std::thread::spawn` を選ぶ理由」節。

## Alternatives Considered

- **全面 `async fn` + `tokio::task::spawn_blocking`**: tokio 依存が増え、`spawn_blocking` の必要性を毎回説明することになる。
- **混在（I/O だけ async、UI は同期）**: 境界で型 / lifetime の衝突が頻発する。
- **CPU-bound だけ thread、I/O は async**: editor のスケールではこの粒度の最適化は不要。

## Consequences

- Repository trait が単純（`Send + Sync` + 同期メソッド）。テスト用の `InMemory*` 実装も書きやすい。
- 重い処理は **必ず `std::thread::spawn` を見えるところに書く** ので、「ここはオフロードしてる」の判別が容易。
- async API 前提のクレート（一部の HTTP / DB クライアント）は そのままでは使えない。必要が出た時点でラッパーを通すか方針再検討する。
- 現状の Repository は呼び出しごとに直接 `std::fs` を叩く。WebView 側 HTTP キャッシュ（→ ADR-0005）に依存しているのでこれで足りている。
