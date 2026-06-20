# 起動と Project 指定

## 起動

```sh
just engine-run                  # alias: en-run         # debug
just engine-run-release          # alias: en-run-rel     # release
```

実体は `cargo run -p engine --bin beatemup` (release 版は `--release`)。

## sample プロジェクトで起動

`runtime/` (private な workspace dir) を持っていない場合は、CC0 プレースホルダー素材で動く `sample-projects/minimal` を `BEATEMUP_RUNTIME_DIR` で渡して起動する:

```sh
just engine-run-sample           # alias: en-run-sample
```

内部的には `BEATEMUP_RUNTIME_DIR=../../sample-projects/minimal cargo run ...` を実行している。プレースホルダー PNG は事前に `just gen-sample` で再生成できる。

## Project の指定

```sh
just engine-run -- --project my-project
just engine-run-sample -- --project=minimal
```

`--project=<name>` フラグ (または `--project <name>`) で `<workspace_dir>/data/projects/{name}.yml` を指定する。フラグ未指定時は `BEATEMUP_PROJECT` 環境変数を見る。どちらもなければ project ロードはスキップされ、Title scene のみが立ち上がる。

## ビルド

```sh
just engine-build                # alias: en-build
```

`cargo build --release` 後、`target/release/beatemup(.exe)` を `runtime/build/` 配下にコピーする。
