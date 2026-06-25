# Bebeu

*The name is **BE**vy + **B**eat **E**m **U**p.*

A Rust beat-em-up engine (Bevy) + visual editor (Dioxus desktop), distributed as a workspace.

For now, this is a personal hobby project, shared in the open in case anyone finds it useful — no general-framework ambitions and no support SLA. Direction may evolve as the project matures.

![sample-projects/minimal running with placeholder sprites](docs/screenshot.png)

> 日本語版は下にあります。

## Status

Early scaffolding. APIs, data formats, and the editor UI are all still in motion. Expect breaking changes.

## Contributing

Issues and bug reports are welcome.
Pull requests are currently invite-only — please open an issue first
so we can talk about the change before you spend time on it.

## Platforms

Developed on Windows 11. Bevy and Dioxus both support macOS and Linux natively,
so the project should build and run on those platforms — it just hasn't been
verified on them yet. For platform-specific system packages, see:

- [Bevy Linux dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md)
- [Dioxus desktop setup](https://dioxus.dev/learn/0.7/getting_started/)

## Workspace layout

```
packages/
  engine/          # Bevy-based runtime (binary: beatemup)
  editor-desktop/  # Dioxus desktop editor for authoring projects
tools/
  asset-gen/       # CLI that writes the placeholder PNGs under sample-projects/
sample-projects/
  minimal/         # CC0 placeholder project (hero vs. enemy on a training stage)
docsite/           # VitePress documentation site
```

## Requirements

- Rust 1.96+ (toolchain pinned in `rust-toolchain.toml`)
- `cargo-nextest` (`cargo install cargo-nextest --locked`) for `just test`
- `just` (https://github.com/casey/just) — task runner
- For the editor: Node.js (tailwindcss / daisyui via npm) and `dioxus-cli` (`just editor-desktop-install-cli`)

## Run the sample project

```
just engine-run-sample --project main
```

`engine-run-sample` sets `BEATEMUP_RUNTIME_DIR` to `sample-projects/minimal`,
so the engine reads its character / level / project YAML from that tree. The
placeholder PNGs are committed under that tree, so a fresh clone runs without
any pre-step. (`just gen-sample` regenerates them from `tools/asset-gen` if
you tweak the generator.) The title scene that opens is intentionally empty —
press **Enter** or **Space** to advance to the battle scene.

## Run the editor

```
just editor-desktop-setup    # one-time: dx CLI + npm deps + tailwind
just editor-desktop-dev      # hot reload
```

The editor reads `packages/editor-desktop/bebeu-editor.yml`, whose
`workspace_dir` points at `sample-projects/minimal` in this public build.
Edit any project YAML there and re-run the engine to see the change.

## Bringing your own assets

`sample-projects/minimal` ships only with generated single-color placeholders.
To author a real game, copy `sample-projects/minimal` somewhere outside the
repo, point `BEATEMUP_RUNTIME_DIR` (and the editor's `workspace_dir`) at the
copy, and replace the sprites / sounds / YAML in place.

## Documentation

A VitePress documentation site lives under `docsite/`. To browse locally:

```
just docsite-setup   # one-time: npm install
just docsite-dev     # http://localhost:5173
```

The hosted version lives at **<https://sakai-nako.github.io/Bebeu/>** (deployed from `main` via `.github/workflows/deploy-docsite.yml`). Doc sources sit directly under `docsite/` (see `index.md` and the `engine/` / `editor/` subdirs). Design decisions are recorded as ADRs under [.claude/adr/](.claude/adr/).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

### Sample assets

Everything under `sample-projects/` (including the generated PNGs from
`tools/asset-gen`) is released under
[CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/).

---

# 日本語

*名前は **BE**vy + **B**eat **E**m **U**p から。*

Rust 製のベルトスクロールアクション engine (Bevy) と、それ専用のビジュアル editor (Dioxus desktop) を 1 つの cargo workspace にまとめたものです。

今のところは個人の趣味プロジェクトを公開しているだけで、汎用 framework を目指していたりサポートを保証しているわけではありません。今後の発展次第で位置づけは変わるかも。

> The English version is above.

## 状態

スキャフォールディング段階で、API・データ形式・editor UI とも頻繁に変わります。破壊的変更を許容してください。

## コントリビューション

Issue / バグ報告は歓迎します。
Pull Request は現状 invite 制で運用しているので、まず Issue を立てて
方針をすり合わせてから着手してください。

## プラットフォーム

開発は Windows 11 で行っているが、Bevy / Dioxus とも macOS / Linux は native
サポート対象なので動くはず (ただし未検証)。プラットフォーム別の system package
については以下を参照:

- [Bevy Linux 依存パッケージ](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md)
- [Dioxus desktop セットアップ](https://dioxus.dev/learn/0.7/getting_started/)

## ディレクトリ構成

```
packages/
  engine/          # Bevy ベースの runtime (バイナリ: beatemup)
  editor-desktop/  # プロジェクト編集用の Dioxus desktop エディタ
tools/
  asset-gen/       # sample-projects 配下のプレースホルダー PNG を書き出す CLI
sample-projects/
  minimal/         # CC0 プレースホルダープロジェクト (hero vs. enemy / training ステージ)
docsite/           # VitePress 製ドキュメントサイト
```

## 必要なもの

- Rust 1.96+ (`rust-toolchain.toml` で pin 済み)
- `cargo-nextest` (`cargo install cargo-nextest --locked`) — `just test` 用
- `just` (https://github.com/casey/just) — task runner
- editor 用: Node.js (npm 経由で tailwindcss / daisyui) と `dioxus-cli` (`just editor-desktop-install-cli`)

## サンプル起動

```
just engine-run-sample --project main
```

`engine-run-sample` は `BEATEMUP_RUNTIME_DIR=sample-projects/minimal` を渡して起動するため、engine はそのツリーから character / level / project YAML を読みます。プレースホルダー PNG は `sample-projects/minimal/` 配下に commit 済みなので、clone 直後に追加手順なしで動きます (`tools/asset-gen` を弄った場合は `just gen-sample` で再生成)。最初に出る title scene は空っぽなので、**Enter** か **Space** を押して battle scene に進んでください。

## エディタ起動

```
just editor-desktop-setup    # 初回: dx CLI + npm 依存 + tailwind
just editor-desktop-dev      # hot reload
```

editor は `packages/editor-desktop/bebeu-editor.yml` を読み、`workspace_dir` キーが Public ビルドでは `sample-projects/minimal` を指しています。そこで project YAML を編集して engine を再起動すれば変更が反映されます。

## 自前プロジェクトを作るとき

`sample-projects/minimal` にあるのは生成された単色プレースホルダーだけです。
本物のゲームを作るときは `sample-projects/minimal` を repo の外にコピーし、
`BEATEMUP_RUNTIME_DIR` と editor の `workspace_dir` を新しいパスに向けて、
sprite / sound / YAML をその場で差し替えてください。

## ドキュメント

VitePress 製のドキュメントが `docsite/` にあります。ローカルで閲覧:

```
just docsite-setup   # 初回: npm install
just docsite-dev     # http://localhost:5173
```

公開版は **<https://sakai-nako.github.io/Bebeu/>** で閲覧できます (`main` push 時に `.github/workflows/deploy-docsite.yml` 経由でデプロイ)。ソースは `docsite/` 直下 (`index.md` と `engine/` / `editor/` サブディレクトリ)。設計判断は [.claude/adr/](.claude/adr/) の ADR として番号付きで蓄積しています。

## ライセンス

以下のどちらかを選んで利用できます:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) または http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) または http://opensource.org/licenses/MIT)

明示的に別の意思表示をしない限り、本プロジェクトへ寄せた contribution は
Apache-2.0 ライセンスの定めに従い、上記のデュアルライセンスで提供されたものとして
扱います (追加条件は付きません)。

### サンプル素材

`sample-projects/` 配下 (`tools/asset-gen` の生成物含む) はすべて
[CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/) です。
