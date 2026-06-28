# Bebeu — Claude Code 向けプロジェクト案内

ベルトスクロール beat-em-up の engine + editor を 1 つの cargo workspace にまとめた Rust プロジェクト。

## 全体像

| crate | 役割 |
|-------|------|
| `packages/engine` | Bevy ベースの runtime (`bebeu-engine` バイナリ) |
| `packages/editor-desktop` | Dioxus desktop の編集ツール (workspace_dir 配下の YAML を編集) |
| `tools/asset-gen` | sample-projects 用プレースホルダー PNG ジェネレータ |

## Workspace dir

engine が読むゲームデータは `sample-projects/<name>/` に置く。`BEATEMUP_RUNTIME_DIR` 環境変数でパスを切り替える (`just engine-run-sample` がデフォルトで `sample-projects/minimal` を渡す)。

エディタ側の workspace dir は `packages/editor-desktop/bebeu-editor.yml` の `workspace_dir` キーで指定する。デフォルトは `../../sample-projects/minimal`。

自前プロジェクトを作るときは `sample-projects/minimal` を repo 外にコピーして、`BEATEMUP_RUNTIME_DIR` と `bebeu-editor.yml` を新しいパスに向け直す。

## アーキテクチャ規約 (FSD)

[.claude/adr/0001](.claude/adr/0001-adopt-feature-sliced-design.md) で FSD (Feature-Sliced Design) を採用。詳細は [.claude/docs/fsd.md](.claude/docs/fsd.md)。

ローカル運用ルール (履歴上ハマったところ):
- **slice 内サブスライス禁止** — 集約配下の型は親 slice の `model.rs` に集約する。
- **engine の features は segment 無し** — slice 直下にファイル直書き、`model`/`api` segment を持ち込まない。
- **editor / engine は独立を維持** — 共通化はそれぞれが熟れてから探す。先に共通基盤を作らない。

## 主要 ADR (頻出参照)

- [0011 filesystem-yaml-as-primary-storage](.claude/adr/0011-filesystem-yaml-as-primary-storage.md): YAML をプライマリ永続化形式とする
- [0012 two-tier-configuration-files](.claude/adr/0012-two-tier-configuration-files.md): config の二段構成
- [0016 engine-config-hybrid-placement](.claude/adr/0016-engine-config-hybrid-placement.md): bebeu-engine.yml の配置
- [0017 world-axes-and-25d-projection](.claude/adr/0017-world-axes-and-25d-projection.md): world 座標と 2.5D 投影
- [0022 level-area-trapezoid-or](.claude/adr/0022-level-area-one-side-parallel-trapezoid-or.md): Level.areas は 1 辺平行台形の OR 合成
- [0023 image-pixel-world-screen-unification](.claude/adr/0023-image-pixel-world-screen-unification.md): 画像ピクセル / world / screen の単位統一

ADR は番号付きで [.claude/adr/](.claude/adr/) に蓄積されている (25 本+)。新しい設計判断を入れたらここに 1 本追加する。

## 参照ドキュメント (.claude/docs)

ADR が「個別の決定」を記録するのに対し、`.claude/docs/` は「外部 framework や横断トピックの常設リファレンス」を置く場所。コード生成・改修時に該当ファイルを先に読んでから着手する。

- [fsd.md](.claude/docs/fsd.md) — Feature-Sliced Design の本プロジェクト適用
- [ooui-fsd.md](.claude/docs/ooui-fsd.md) — OOUI を FSD レイヤに落とし込む指針 (editor UI 設計時)
- [data-flow.md](.claude/docs/data-flow.md) — editor 側のデータフロー (load → edit → save の規約)
- [undo-redo.md](.claude/docs/undo-redo.md) — undo/redo の snapshot 戦略 ([ADR-0010](.claude/adr/0010-session-scope-snapshot-history.md))
- [dioxus-reactivity.md](.claude/docs/dioxus-reactivity.md) — Dioxus signal / use_effect / use_memo の挙動メモ
- [daisyui-base.md](.claude/docs/daisyui-base.md), [daisyui-components-1.md](.claude/docs/daisyui-components-1.md), [daisyui-components-2.md](.claude/docs/daisyui-components-2.md) — daisyUI コンポーネントカタログ
- [testing.md](.claude/docs/testing.md) — テスト方針 (nextest / unit / integration)

## 開発フロー

```
just verify              # fmt + clippy + build + test (workspace 全体)
just engine-run-sample   # sample-projects/minimal を engine で起動
just editor-desktop-dev  # editor を hot reload で開発
just gen-sample          # sample-projects のプレースホルダー PNG を再生成
```

その他は `just -l` で確認。テスト runner は nextest (`cargo install cargo-nextest --locked`)。

## 編集ルール

- コメント: 「WHY が非自明な場合のみ」書く。WHAT は識別子で表現する。長文は ADR / docs に書き、コード内では `// ADR-0023` のように番号で参照する。
- エラー処理 / フォールバックは「実際に起きうるシナリオに対してだけ」入れる。内部呼び出しの引数やフレームワーク保証は信頼する。
- 後方互換 hack や `_unused` 残しは避ける。完全に消す。
- バイナリ asset (`*.png` `*.wav` 等) は `sample-projects/` 配下のみ commit。
