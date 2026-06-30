set quiet
set windows-shell := ["sh", "-uc"]

export NEXTEST_NO_TESTS := "pass"

[private]
default:
    just -l --unsorted



# --- workspace 全体 ---

# 依存解決のみ (CI 用)。fetch だけ走らせる。
setup:
    cargo fetch
alias s := setup

# 全 crate フォーマット。
fmt:
    cargo fmt --all

# 全 crate clippy。-D warnings はあえて付けない (まずは雛形を通す)。
check:
    cargo clippy --workspace --all-targets

# 全 crate ビルド。
build:
    cargo build --workspace

# 全 crate テスト。nextest を使う (`cargo install cargo-nextest --locked` で入れる)。
test *args:
    cargo nextest run --workspace {{ args }}

# verify: fmt + clippy + build + test。
verify: fmt check build test



# --- engine (bevy) ---

# engine 起動系は packages/engine を CWD にする (asset 探索が CWD 基準のため)。
[group("engine")]
[working-directory: "packages/engine"]
engine-run *args:
    cargo run -p bebeu-engine --bin bebeu-engine -- {{ args }}
alias en-run := engine-run

[group("engine")]
[working-directory: "packages/engine"]
engine-run-release *args:
    cargo run --release -p bebeu-engine --bin bebeu-engine -- {{ args }}
alias en-run-rel := engine-run-release

[group("engine")]
engine-test *args:
    cargo nextest run -p bebeu-engine {{ args }}
alias en-test := engine-test

# sample-projects/minimal を engine で起動 (CC0 placeholder で動く、--project main で battle scene へ)。
[group("engine")]
[working-directory: "packages/engine"]
engine-run-sample *args:
    BEATEMUP_RUNTIME_DIR=../../sample-projects/minimal cargo run -p bebeu-engine --bin bebeu-engine -- {{ args }}
alias en-run-sample := engine-run-sample



# --- sample-projects ---

# sample-projects/minimal にプレースホルダー PNG を生成 (idempotent)。
gen-sample:
    cargo run -p asset-gen -- sample-projects/minimal
alias gen := gen-sample



# --- editor-desktop (dioxus desktop) ---

# dioxus-cli (dx) を ^0.7 で install (hot reload / bundle に使う)。
[group("editor-desktop")]
editor-desktop-install-cli:
    cargo install dioxus-cli --version '^0.7' --locked
alias ed-d-install-cli := editor-desktop-install-cli

# 初回セットアップ。dx install + npm 依存 + assets/tailwind.css 生成 (asset! が compile 時に存在チェックするため必須)。
[group("editor-desktop")]
[working-directory: "packages/editor-desktop"]
editor-desktop-setup: editor-desktop-install-cli
    npm install
    npx @tailwindcss/cli -i tailwind.css -o assets/tailwind.css --minify
alias ed-d-setup := editor-desktop-setup

[group("editor-desktop")]
[working-directory: "packages/editor-desktop"]
editor-desktop-deps:
    [ -d node_modules ] || npm install

[group("editor-desktop")]
[working-directory: "packages/editor-desktop"]
editor-desktop-css: editor-desktop-deps
    npx @tailwindcss/cli -i tailwind.css -o assets/tailwind.css --minify
alias ed-d-css := editor-desktop-css

# 開発用。dx serve で hot reload。新規 Tailwind class を追加したら
# 別 shell で `just ed-d-css` を 1 回回す (assets/tailwind.css 更新で hot reload)。
[group("editor-desktop")]
[working-directory: "packages/editor-desktop"]
editor-desktop-dev *args: editor-desktop-deps
    dx serve --platform desktop {{ args }}
alias ed-d-dev := editor-desktop-dev

[group("editor-desktop")]
[working-directory: "packages/editor-desktop"]
editor-desktop-run *args: editor-desktop-css
    cargo run -p bebeu-editor-desktop -- {{ args }}
alias ed-d-run := editor-desktop-run

[group("editor-desktop")]
[working-directory: "packages/editor-desktop"]
editor-desktop-run-release *args: editor-desktop-css
    cargo run --release -p bebeu-editor-desktop -- {{ args }}
alias ed-d-run-rel := editor-desktop-run-release

[group("editor-desktop")]
[working-directory: "packages/editor-desktop"]
editor-desktop-build *args: editor-desktop-css
    dx build --platform desktop --release {{ args }}
alias ed-d-build := editor-desktop-build

[group("editor-desktop")]
[working-directory: "packages/editor-desktop"]
editor-desktop-bundle *args: editor-desktop-css
    dx bundle --platform desktop --release {{ args }}
alias ed-d-bundle := editor-desktop-bundle

[group("editor-desktop")]
editor-desktop-test *args:
    cargo nextest run -p bebeu-editor-desktop {{ args }}
alias ed-d-test := editor-desktop-test



# --- docsite (VitePress) ---

[group("docsite")]
[working-directory: "docsite"]
docsite-setup:
    npm install
alias ds-setup := docsite-setup

[group("docsite")]
[working-directory: "docsite"]
docsite-dev:
    npm run dev
alias ds-dev := docsite-dev

[group("docsite")]
[working-directory: "docsite"]
docsite-build:
    npm run build
alias ds-build := docsite-build

[group("docsite")]
[working-directory: "docsite"]
docsite-preview:
    npm run preview
alias ds-preview := docsite-preview
