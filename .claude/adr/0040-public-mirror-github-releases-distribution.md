# ADR-0040: Public mirror GitHub Releases zip 配布の構造

## Status

Accepted (2026-06-28、Issue #18 で適用)

## Context

itch.io ページ初版公開のタイミングで、Public mirror (`sakai-nako/Bebeu`, GitHub)
の Releases から zip を配れるようにしたい。itch.io が「触りたい一般ユーザー」
向けの surface だとすると、GitHub Releases は「コード読む / fork する技術者」
向け = Bevy / Rust gamedev コミュニティ (Punchy / HillVacuum / bevy-assets リスト
経由の流入) の慣行に合わせる。

配布対象は **engine + editor + sample-projects/minimal の同梱 zip**。理由:

- engine binary 単体は workspace_dir 解決が compile-time `CARGO_MANIFEST_DIR`
  fallback で死ぬ ([engine/shared/config.rs:37-43](../../packages/engine/src/shared/config.rs))。
  どこかに `sample-projects/minimal` 相当のデータと、それを指す env / yml が必要
- editor 単体も同じ理由で workspace_dir が無いと dialog が開く ([editor-desktop/shared/config.rs:43-50](../../packages/editor-desktop/src/shared/config.rs))
- Punchy が `cargo install` 経由配布をしていない事実通り、Rust gamedev 配布は
  「zip 展開して exe ダブルクリック」が一次形式

## Decision

### platform

**Windows のみ** で v0.1.0 開始。Issue 本文の判断どおり、macOS / Linux は
matrix 追加コストに対し初期需要が読めないため後送り。Public mirror の README
で "help wanted" として募集する。

Bevy / Dioxus は両 OS native 対応なのでビルド自体は通る想定だが、cross-build
の動作確認・コード署名・配布形式 (.app / .deb / AppImage) の判断は別 Issue で扱う。

### workflow 配置

**`tools/public-overlay/.github/workflows/release.yml`**。本 repo の `.github/`
は mirror tool の `apply_overlay` 段階で `tools/public-overlay/.github/` から
コピーされる構造 ([reference: public_overlay_replacement])。`deploy-docsite.yml`
が既に同パターン。ローカル repo 側に `.github/` を生やさない。

### build cmd

| crate | cmd | 出力 |
|-------|-----|------|
| bebeu-engine | `cargo build --release -p bebeu-engine --bin bebeu-engine` | `target/release/bebeu-engine.exe` |
| bebeu-editor-desktop | `dx build --platform desktop --release` (cwd: `packages/editor-desktop`) | `target/dx/bebeu-editor-desktop/release/desktop/` (exe + assets/ + DLL) |

`dx bundle` は msi / .app / .deb など OS native installer を作るのが主目的。
zip 展開 + ダブルクリック起動を想定する本配布では不要。`dx build` の出力を
そのまま zip に詰める。

### zip layout

```
Bebeu-v0.1.0-windows-x64/
  bebeu-engine.exe                       # engine
  bebeu-editor-desktop.exe + assets/ + *.dll   # dx build 出力一式
  bebeu-editor.yml                       # workspace_dir: sample-projects/minimal (相対)
  sample-projects/minimal/               # CC0 placeholder project
  run-engine.cmd                         # cd /d %~dp0 + set BEATEMUP_RUNTIME_DIR + bebeu-engine.exe
  run-editor.cmd                         # cd /d %~dp0 + bebeu-editor-desktop.exe (yml 自動)
  README.txt
  LICENSE-MIT
  LICENSE-APACHE
```

### launcher `.cmd` + 同梱 yml が必須な理由

zip 展開先のパスは builder PC とは違うため、以下が必要:

- `cd /d %~dp0` で CWD を zip ルートに固定 → Bevy `AssetServer` の CWD 基準解決と
  engine の env var 相対パス解釈の両方を担保
- `set BEATEMUP_RUNTIME_DIR=sample-projects\minimal` で engine の compile-time
  `CARGO_MANIFEST_DIR` fallback を回避 (これを踏むと builder PC の絶対パスを見に行く)
- editor は exe 隣の `bebeu-editor.yml` を `current_exe().parent()` 経由で見つける
  ([editor-desktop/shared/config.rs:29](../../packages/editor-desktop/src/shared/config.rs))
  ので、env var 不要・yml 同梱で完結

### release notes / draft 運用

`softprops/action-gh-release@v2` で **draft release** を作成 (`draft: true`)。
`generate_release_notes: true` で commit log ベースの初期 notes を入れ、ユーザーが
内容確認後に手動で publish。`v*` tag push と `workflow_dispatch` (任意 tag 名) の
両方をトリガーにする。

## Consequences

- v0.1.0 タグ切り 1 回で zip 自動生成、以降の release も同じ動線で続けられる
- ローカル repo 側に `.github/` が無いのは継続 (docsite workflow と同じ)
- 配布 zip に `bebeu-engine.yml` (window size 等) は同梱しない: config 解決経路
  に compile-time MANIFEST_DIR の罠があり、window size は `EngineConfig::default()`
  からの fallback で動くため初版では割愛。同梱方式が必要になったら別 Issue で
  config 解決の env 主導化と合わせて検討
- 補助 file (launcher / README / 同梱 yml) は **workflow YAML 内に inline** で
  生成する。`tools/public-overlay/release/` のような新 dir を切ると、mirror tool
  の overlay 仕様で mirror 先 repo root に `release/` が出てしまう ([reference:
  public_overlay_replacement]) のを避ける

### Yaranai (別 Issue に escalate)

- **macOS / Linux ビルド追加** (matrix 化、コード署名、配布形式判断)
- **itch.io への butler push 自動化** (`butler push <zip> sakai-nako/bebeu:windows`)
  — 初版は手動で zip を両方に upload
- **crates.io publish** — `bebeu-engine` / `bebeu-editor-desktop` は binary crate なので
  publish 需要は薄い。library use case (Bevy plugin として組み込む) が出てきたら
  別 Issue で起こす
- **自動 tag 化 / リリースサイクル** — semver bump の自動化、changelog 生成等は
  リリース頻度が見えてから

## References

- Issue #18 (本 ADR の発端)
- Refs #12 (marketing umbrella、itch.io 公開動線と並列)
- ADR-0016 (`bebeu-engine.yml` 配置) — 同梱しない判断の根拠
- 既存 `tools/public-overlay/.github/workflows/deploy-docsite.yml` (同型パターン)
- [marketing/research/bevy-devlog-survey.md](../../marketing/research/bevy-devlog-survey.md)
  — bevy-assets リスト / Punchy 観察
