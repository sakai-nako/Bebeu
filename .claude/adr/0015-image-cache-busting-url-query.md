---
name: ADR-0015 — Image cache busting via URL query
description: 同じ basename で画像を上書きしたときに WebView のキャッシュを URL `?v={N}` クエリで再フェッチさせる決定
type: adr
---

# ADR-0015: 画像 URL の cache busting を `?v={N}` クエリで行う

## Status

Accepted

## Context

ADR-0005 で workspace 配下の画像レスポンスに `Cache-Control: max-age=3600` を付け、WebView 側に 1 時間キャッシュさせている。読み取りだけなら問題ないが、editor は実際には画像を編集する:

- **`reimport_sprites_scaled`**: 既存 `sprite-groups/{group}/sprites/*.png` を別倍率で再 import して **同じ basename** で上書きする
- **`change_thumbnail`**: `sprite-groups/thumbnail/sprites/{character_name}.png` を上書きする
- **`import_sprites`**: 同名 PNG を再 import すると上書きになる

このとき URL は変わらないため、WebView は 1 時間古い画像を返し続ける。サイドバーの thumbnail、detail のスプライト一覧、Animation Editor の各 frame など、複数箇所で同じ URL を参照しているので「リロードしても古いまま」が体感的にも顕著になる。

ADR-0005 の Consequences でも「書き換え機能を入れたら 3 案のいずれかに切り替える」と前提条件付きで残してあった。今がその時点。

## Decision

**プロセスローカルなカウンタ `ImageCacheBuster(u64)` を Dioxus context で配布し、画像 URL の末尾に `?v={N}` を付与する。書き換え操作の直後に `bump()` でカウンタを進めると、配下の画像コンポーネントが新 URL で再フェッチする。**

実装 (`shared/image_cache_buster.rs`):

```rust
pub struct ImageCacheBuster(pub u64);

pub fn use_image_cache_buster_provider() -> Signal<ImageCacheBuster>;
pub fn use_image_cache_buster() -> Option<Signal<ImageCacheBuster>>;
pub fn versioned_asset_url(url: String, version: u64) -> String;
```

規約:

- **配布スコープは編集サーフェス単位**: `SpriteGroupEditor` などの「画像を書き換える可能性がある」widget で `use_image_cache_buster_provider()` を呼んで Signal を提供する。配下の画像コンポーネント (`SpriteThumbnail` 等) は `use_image_cache_buster()` で読む。
- **provider 不在時は no-op**: `try_consume_context` で取得して `Option<Signal<_>>` を返す。`versioned_asset_url(url, 0)` も素の URL をそのまま返す。これにより Editor 外の一覧画面 (Sidebar の thumbnail 等) でもコンポーネントを再利用できる。
- **`bump()` は書き換えに成功した直後に呼ぶ**: feature 側 (`reimport_sprites_scaled`, `change_thumbnail` 等) で disk への書き込みが終わった後に `cache_buster.write().bump()`。
- **オーバーフロー処理**: `wrapping_add(1)`。`u64` を 1 編集 1 bump で使い切るシナリオは現実には来ないが、安全側に倒す。

## Alternatives Considered

- **`Cache-Control: no-cache` に切替** (ADR-0005 で挙げていた案 2):
    - asset_handler 全体に効くので、編集しない sprite まで毎レンダ fs read する。loopback でもキャッシュヒット時のコスト 0 を捨てるのは惜しい。
    - URL をハンドラ内で見て「編集対象だけ no-cache」にする条件分岐は責務が逆 (resolver がドメイン的な編集状態を知らないと書けない)。

- **WebView キャッシュを明示的に flush** (案 3):
    - wry の API で部分的な invalidate を狙うことになるが、URL 単位の細かい制御は提供されていない。「全 flush」は他の画像にも巻き添えで効く。
    - プラットフォーム差異 (Windows / macOS / Linux で WebView 実装が違う) が API 表面に出やすい。

- **`?v={mtime}` (ファイルの mtime を使う)**:
    - 「実際に変わった画像だけ URL が変わる」のが一見素直に見えるが、画像コンポーネント側で `<img>` を描画する時点では「自分が指している disk ファイル」を知るために feature 層と非対称な依存が必要 (entity から img 表示までの経路に metadata 取得を割り込ませる)。
    - mtime を読むには毎回 syscall。表示数が多い Animation Editor で効く。
    - 編集 feature が「自分が書き換えた」を知っているのだから、書き換え側が能動的に bump するほうが情報の出所として自然。

- **画像コンポーネントを `key` で remount**:
    - Dioxus の差分検知に介入することになり、scroll 位置・focus などの副次状態が巻き添えで吹き飛ぶ。`?v=` は DOM 上は同じ要素のまま `src` だけ変わるので副作用が少ない。

- **画像 basename を毎回ユニークにする (`portrait_001_v2.png` 等)**:
    - YAML 側に書かれた basename を書き換えると、import 機能の単純性 (元ファイル名をそのまま使う) が崩れる。
    - 旧ファイルを残すか消すかの新たな問題が生まれる。

- **provider を root で常に提供**:
    - 配布範囲を広げる利点はあるが、Sidebar の thumbnail のような「編集と関係ない場所」まで bump で巻き添え再フェッチさせることになる。今の「編集サーフェス単位で provide、外では no-op」は責務が局所化されていて好ましい。

## Consequences

**得られたもの**

- ADR-0005 の cache 設計を温存したまま、書き換え時だけ最新を取り戻せる。Rust 側に in-memory cache を持たない原則も維持。
- `bump()` を呼ぶのは feature 層 (write API を呼ぶ場所と同じ層) なので、責任の所在が明確。entity / widget は無関係。
- provider 不在時 no-op により、画像コンポーネントを Editor 外でも条件分岐なしで使い回せる。

**支払うコスト / 注意点**

- **bump 忘れ = 古い画像が残る**: 新しい画像書き換え系 feature を追加するときは `bump()` の呼び出しを忘れない。レビュー観点として明文化する必要がある。
- **provider のスコープ漏れ**: 編集機能を持つ widget が `use_image_cache_buster_provider` を呼び忘れると、その widget 配下の画像が `bump()` を受け取れず古いまま。新規 editor を追加するときの注意点。
- **URL クエリ違いは別エントリ扱い**: WebView から見ると `foo.png` と `foo.png?v=1` は別キャッシュエントリ。bump するたび古いエントリが 1 時間キャッシュに残る (TTL で消える)。書き換え頻度が極端に高い場合のメモリ圧迫はあり得るが、editor の実用範囲では問題にならない。

**今後の拡張余地**

- 「どの画像が更新されたか」を絞り込む粒度を上げたい場合、`HashMap<PathBuf, u64>` 化して basename / sprite_group 単位の bump にできる。現状は「一括 invalidate で十分」と判断している。
- `?v={N}` の N に意味を持たせず単調増加させているので、将来 mtime や hash に切り替えるのも互換性を壊さず可能 (URL 規約だけが対外契約)。

## 関連 ADR / リファレンス

- ADR-0005: WebView asset handler with 1h cache (この ADR が前提とするキャッシュ機構の設計)
- `packages/editor/src/shared/image_cache_buster.rs`: 実装本体
- `packages/editor/src/shared/README.md`: shared モジュール一覧
