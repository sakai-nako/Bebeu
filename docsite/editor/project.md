# Project / Character / Level

Character と Level は `<workspace_dir>/data/characters/` / `<workspace_dir>/data/levels/` のマスタープールで管理し、Project は `<workspace_dir>/data/projects/{name}.yml` で「どの Character / Level をどの役割で使うか」「論理解像度はいくつか」「HUD をどう配置するか」を表す。

- Project は players / opponents / levels の役割配列、`resolution`、`hud:` セクションを持つ
- 同じ Character / Level を複数の Project から参照できる
- engine はウィンドウサイズ等のランタイム設定を `packages/engine/bebeu-engine.yml` から読む (ADR-0016)

## YAML フィールド

| フィールド | 単位 / 既定値 | 役割 |
|---|---|---|
| `resolution` | `{ width, height }` (px) / `{ 640, 360 }` | 描画バッファの論理解像度 (ADR-0026 で中間 render texture に使われる) |
| `players` | 文字列リスト / 空 | Player 役割の Character 名。先頭から P1, P2, ... に割り当てられる |
| `opponents` | 文字列リスト / 空 | Opponent 役割の Character 名 (敵キャラのプール) |
| `levels` | 文字列リスト / 空 | この Project で使用する Level 名 |
| `hud` | `{ elements: [...] }` / 空 | gameplay 中の HUD レイアウト (詳細は [HUD レイアウト](./hud.md)) |

`name` フィールドはファイル名 stem から復元されるので YAML には書かない (`#[serde(skip)]`)。

## エディタ上での編集

Project 詳細ページ (`/projects/{name}`) には以下のカードが並ぶ:

1. **Resolution** — 論理解像度の width / height
2. **Players / Opponents / Levels** — 3 つのマルチセレクトで Character / Level のマスタープールから役割を組み立てる
3. **HUD レイアウト** — HUD 要素の追加・編集・並べ替え (詳細は [HUD レイアウト](./hud.md))
4. **engine 起動** — 「engine 起動コマンドを表示」ボタンで `just engine-run -- --project {name}` をモーダル表示。コピーしてターミナルで叩く想定。players / opponents / levels が空の Project ではボタンが無効化される

Save するまでは disk に書き込まれない。
