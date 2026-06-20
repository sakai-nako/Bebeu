# engine

Bevy で作られたランタイム (`beatemup` バイナリ)。editor で作成した Project（`<workspace_dir>/data/projects/{name}.yml`）を読み込んで動作する。

workspace dir は `BEATEMUP_RUNTIME_DIR` 環境変数で指定する。未設定時は `packages/engine/../../runtime` を見るので、最初に試すときは `just engine-run-sample` で `sample-projects/minimal` を渡すのが手早い。

## 構成

- [起動と Project 指定](./run.md) — `engine-run` / `engine-run-sample` / `--project` フラグ / `BEATEMUP_PROJECT` 環境変数
- [操作方法](./controls.md) — プレイヤー操作のキーバインド
- [Debug ビルド](./debug.md) — debug ビルドと hitbox overlay の現状
