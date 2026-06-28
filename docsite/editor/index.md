# editor

Dioxus desktop で作られたエディタ (`packages/editor-desktop`)。workspace dir 配下の `data/` の YAML を読み書きして、Project / Character / Level を編集する。

このセクションでは、エディタを起動してから Project を編集するまでの一連の操作を扱う。

## 構成

- [セットアップと起動](./setup.md) — 初回セットアップと開発サーバの起動
- [Workspace の選択](./workspace.md) — `bebeu-editor.yml` でどの workspace dir を開くか
- [Project / Character / Level](./project.md) — マスタープールと Project の役割配列
- [Level](./level.md) — Level YAML スキーマと編集フロー
- [HUD レイアウト](./hud.md) — Project YAML の `hud:` セクション (HP バー / リング / 敵バーの配置)
- [キーバインド](./keybindings.md) — エディタ内で利用できるショートカット
- [ユーザー設定](./preferences.md) — テーマなど OS 標準パスに保存される設定
