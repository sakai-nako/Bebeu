# Project / Character / Level

Character と Level は `<workspace_dir>/data/characters/` / `<workspace_dir>/data/levels/` のマスタープールで管理し、Project は `<workspace_dir>/data/projects/{name}.yml` で「どの Character / Level をどの役割で使うか」を表す役割配列を持つ。

- Project は players / opponents / levels の役割配列を保持する
- 同じ Character / Level を複数の Project から参照できる
- engine はウィンドウサイズなどランタイム設定を `packages/engine/bebeu-engine.yml` から読む (ADR-0016)
