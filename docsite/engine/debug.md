# Debug ビルド

`just engine-run` (alias: `en-run`) は debug profile (`cargo run`) で起動する。release を使いたい場合は `just engine-run-release` (alias: `en-run-rel`)。

```sh
just engine-run                # debug    (cargo run)
just engine-run-release        # release  (cargo run --release)
```

## ログレベル

既定の log filter は `wgpu=error,wgpu_core=error,wgpu_hal=error,naga=warn,info`。Bevy `LogPlugin` の仕様により `RUST_LOG` を設定するとそちらが優先される。

```sh
RUST_LOG=debug just engine-run
```

## hitbox overlay

`packages/engine/src/features/character/hitbox_debug.rs` に雛形だけがある段階で、現状の build には overlay は組み込まれていない (F1 切替は将来実装予定)。
