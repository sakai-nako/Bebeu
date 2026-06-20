# entities/project

workspace 配下の **複数 Project** を扱う entity slice。

## 役割

1 workspace に複数 Project を持てる。各 Project は `workspace/data/projects/{name}.yml` に永続化される (Character / Level と同じ flat YAML 規約)。

Project は engine 起動の **preset** として機能する。`players` / `opponents` / `levels` に YAML で並べた中から engine 側が `[0]` を pick して起動する (Phase 1)。Editor 上の Character / Level 一覧は **master pool 全件**を表示し、Project の選択でフィルタすることはしない (= Project と Character の作成順序はどちらが先でも構わない)。

```yaml
# workspace/data/projects/my-project.yml
resolution:
  width: 640
  height: 360
players:
  - MooR_01
opponents:
  - MooR_02
levels:
  - ct
```

## Editor は active Project を持たない

Editor 内に「アクティブ Project」概念は持たせない。Project 詳細ページ (`/projects/:name`) は URL の名前で直接 Repository から get する。

engine 起動時の Project 選択は engine 側で完結する:

- `--project <name>` flag (もしくは `BEATEMUP_PROJECT` 環境変数) 指定 → そのまま使う
- 未指定で `workspace/data/projects/` が 1 件 → auto-select
- 未指定で 2 件以上 → engine が起動時に対話プロンプト (stdin) で選択させる
- 未指定で 0 件 → legacy `engine/config/game.yml` 経路にフォールバック

詳細は [packages/engine/internal/app/project_select.go](../../../../engine/internal/app/project_select.go)。

## Repository

`ProjectRepository` trait は Character の repository pattern を踏襲し以下を提供する:

| メソッド | 動作 |
|---|---|
| `list()` | 名前一覧 (sorted) |
| `get(name)` | 不在で error |
| `create(project)` | 既存名で error |
| `update(project)` | 不在で error |
| `rename(old, new)` | new 既存 / old 不在で error |
| `delete(name)` | 不在で error |

実装は `FilesystemProjectRepository` (workspace/data/projects/ 直下) と、テスト用の `InMemoryProjectRepository`。

## engine との連動

engine 起動は editor の設定ファイルに**書き込まない**設計。代わりに engine が `--project <name>` flag (もしくは `BEATEMUP_PROJECT` 環境変数) を受け取り、`workspace/data/projects/{name}.yml` を直接読む ([packages/engine/internal/entities/project/loader.go::LoadByName](../../../../engine/internal/entities/project/loader.go))。

Editor は「engine 起動コマンドを表示」ボタン ([features/project/ui/launch_engine_button.rs](../../features/project/ui/launch_engine_button.rs)) で `just engine-run --project <name>` のコマンドをユーザーに見せるだけで、実プロセスは scope 外。将来 editor / engine が別バイナリで配布される場合も、editor は Project YAML を書くだけで済むので破綻しない。

## 旧形式の扱い

旧 `workspace/data/project.yml` (resolution のみ持つ singleton) は廃止。AppMain 起動時に旧ファイルが残っていて新ディレクトリ `data/projects/` が空なら warn ログを出すだけで、auto-migration はしない。手動で `data/projects/<任意の名前>.yml` にリネーム移動する。

engine 側は `--project` 未指定時の legacy fallback として旧形式 + `engine/config/game.yml` を読む経路を当面残す (Phase 2 以降で削除)。

## 参考ファイル

- [model.rs](model.rs): `Project { resolution, players, opponents, levels }`
- [api.rs](api.rs): `ProjectRepository` trait と `FilesystemProjectRepository` / `InMemoryProjectRepository`
- [provider.rs](provider.rs): `ActiveProject` struct と Signal context 配布
- [refresh.rs](refresh.rs): `ProjectsRefreshTrigger` (CRUD 後の一覧再評価)
