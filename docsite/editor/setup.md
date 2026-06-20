# セットアップと起動

## 初回セットアップ

```sh
just editor-desktop-setup   # alias: ed-d-setup
```

dioxus-cli (`dx`) のインストール → `npm install` (tailwindcss / daisyui) → `assets/tailwind.css` の生成、を順に実行する。

## 開発サーバの起動 (hot reload)

```sh
just editor-desktop-dev     # alias: ed-d-dev
```

tailwind の `--watch` ビルドを並行で立ち上げつつ、`dx serve --platform desktop` でホットリロード付きの開発ビルドを起動する。

## 単発起動 (hot reload なし)

```sh
just editor-desktop-run     # alias: ed-d-run
```

`dx` を介さず `cargo run -p editor-desktop` で起動する。CSS だけ事前に rebuild する。

## release ビルド / 配布パッケージ

```sh
just editor-desktop-build   # alias: ed-d-build    # asset 込みで release ビルド (target/dx/.../desktop/)
just editor-desktop-bundle  # alias: ed-d-bundle   # Dioxus.toml [bundle] の設定に従って配布パッケージを生成
```
