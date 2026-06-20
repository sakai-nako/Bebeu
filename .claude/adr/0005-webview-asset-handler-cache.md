# ADR-0005: WebView asset handler with 1-hour `Cache-Control`

## Status

Accepted

## Context

エディタは workspace 配下のユーザーデータ（sprite PNG, thumbnail 等）を `<img src="...">` で表示する。`asset!()` マクロは静的アセット用なので、実行時にユーザーが編集する workspace データには使えない。

何も対策しないと:

- 同じ画像が複数箇所（サイドバー / detail / アニメーションフレーム）に出るたびに `std::fs::read` が走る
- Rust 側で Vec<u8> キャッシュを持つと、画像枚数 × 平均サイズのメモリを抱える

しかも desktop なので「キャッシュコストはほぼゼロ、転送はループバック」という特殊な条件がある。

## Decision

`use_workspace_asset_handler` に `WORKSPACE_ASSET_SCHEME` のリクエストを処理させる:

- URL `/workspace-asset/{relpath}` を `{workspace_dir}/{relpath}` に解決
- canonicalize 後に workspace_dir 配下にあるかを検証（path traversal 対策）
- レスポンスに `Cache-Control: max-age=3600` を付与
- Rust 側は in-memory キャッシュを **持たない**

WebView (wry → Chromium) の HTTP キャッシュが 1 時間だけバイト列を保持する。実装は `app/asset_handler.rs`。

## Alternatives Considered

- **`asset!()` マクロ**: ビルド時バンドル。ユーザーデータには使えない。
- **data URL**: base64 化で DOM が肥大化、ブラウザのキャッシュも効かない。
- **Rust 側 in-memory cache**: WebView 側のキャッシュと二重化。メモリコストの割に得が少ない。
- **`Cache-Control: no-cache`**: 毎レンダで fs read。loopback とはいえ syscall コストはゼロではない。

## Consequences

- 画像の繰り返し表示はほぼ無料（WebView がメモリから返す）。
- **キャッシュ起因の落とし穴**: 同じ URL で内容を上書きすると 1 時間古いまま。書き換え機能 (`reimport_sprites_scaled` 等) を入れた段階で **ADR-0015** に従い、案 1（URL に `?v={N}` クエリを付与する `ImageCacheBuster`）を採用済み。代替案 (`Cache-Control: no-cache` への切替 / WebView キャッシュ flush) は ADR-0015 の Alternatives に経緯を残してある。
- canonicalize で path traversal は塞がっている（canonicalize 失敗または workspace 配下外なら 404）。
