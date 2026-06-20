# Workspace の選択

エディタはどの workspace dir を開くかを `bebeu-editor.yml` の `workspace_dir` キーで決める。読み込み位置は build profile によって変わる:

- **debug ビルド**: カレントディレクトリの `bebeu-editor.yml` (justfile の `editor-desktop-*` レシピは `packages/editor-desktop/` を CWD にして起動する)
- **release ビルド**: 実行ファイルと同じディレクトリの `bebeu-editor.yml`

設定ファイルが見つからなければ起動時にフォルダ選択ダイアログが出て、選んだディレクトリへ新規に書き出される。

```yaml
# packages/editor-desktop/bebeu-editor.yml (リポジトリ同梱の既定値)
workspace_dir: ../../sample-projects/minimal
```

workspace dir の構造:

```
<workspace_dir>/
└── data/
    ├── characters/   ← マスタープール（全 Project 共有）
    ├── levels/       ← マスタープール（全 Project 共有）
    └── projects/     ← Project ごとの YAML
```

既定では `sample-projects/minimal` がそのまま workspace dir として機能する (CC0 プレースホルダー素材で動く最小プロジェクト)。自前プロジェクトを作るときは `sample-projects/minimal` を repo の外にコピーして、`workspace_dir` を新しいパスへ向け直す。
