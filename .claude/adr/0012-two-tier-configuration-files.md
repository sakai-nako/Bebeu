# ADR-0012: 設定ファイルを「workspace pointer」と「user preferences」の二層に分ける

## Status

Accepted

## Context

editor が起動時 / 実行中に必要とする設定情報が 2 種類ある:

1. **workspace の場所**: editor が読み書きする `workspace/` ディレクトリのパス。プロジェクトごとに違う。複数 workspace を切り替える運用も想定（同一 OS ユーザーが複数の制作プロジェクトを持つ）
2. **ユーザー設定**: テーマ、キーバインド、ビュー操作（パン/ズーム）、Undo 履歴上限など。OS ユーザーごとに固定で、workspace を切り替えても保ちたい

これを 1 ファイルに混ぜると、

- workspace 切替のたびにテーマ等が初期化される（最悪）
- workspace を Git に push するとユーザーごとのテーマがチームで共有される（プライバシー / 環境差の問題）
- 別マシンで同じ workspace を開くとそのマシンのテーマが上書きされる

要件:

- workspace pointer は **「この exe / 起動位置がどの workspace を指すか」** を表現する
- user preferences は **「この OS ユーザーがどう editor を使いたいか」** を表現する
- 両者の保存場所は性質が違うので別ファイルにする
- 不在 / 壊れている時のフォールバック挙動が両者で違う（workspace pointer は「無いと起動できない」、preferences は「無くても default で動く」）

## Decision

### 2 ファイルに分ける

| ファイル | 役割 | 配置 | 不在時の挙動 |
|---|---|---|---|
| `local-gw.yml` | workspace pointer | debug: CWD / release: exe の隣 | FileDialog で workspace を選ばせて作成 |
| `preferences.yml` | user preferences | OS 標準 `config_dir/local-game-editor/` | `Preferences::default()` を返す（fail-soft） |

### `local-gw.yml`（workspace pointer）

- 中身は `workspace_dir: PathBuf` 1 フィールドのみ
- debug build: `std::env::current_dir().join("local-gw.yml")` を見る。リポジトリ直下に置いて `.gitignore` する運用
- release build: `std::env::current_exe().parent().join("local-gw.yml")` を見る。配布物 exe の隣に置く運用
- 不在なら起動時に FileDialog で「設定ディレクトリ」と「workspace ディレクトリ」を順に選ばせ、その場で書き込む（`shared/config.rs::Config::load`）
- 起動の前提条件なので、読み込み失敗 = アプリ起動失敗

### `preferences.yml`（user preferences）

- 配置: `dirs::config_dir().join("local-game-editor").join("preferences.yml")`
  - Windows: `%APPDATA%\local-game-editor\preferences.yml`
  - macOS: `~/Library/Application Support/local-game-editor/preferences.yml`
  - Linux: `~/.config/local-game-editor/preferences.yml`
- 不在 / parse 失敗のいずれも warn ログを出して `Preferences::default()` を返す（`entities/preference/api.rs::FilesystemPreferencesRepository::load`）
- 既存 yml に新規フィールドが追加されたケースは `#[serde(default)]` でフィールド単位に default 補完する（`KeyBindings` 追加時の前方互換）

#### 補追 (2026-06-29、ADR-0042): `locale: Locale` フィールド追加

editor UI の表示言語を表す `locale: Locale` (Ja / En) を `Preferences` に追加した
(Issue #30 Phase 1)。`#[serde(default)]` で既存 yml は `Locale::default()` = `Ja` に補完される
(既存ユーザーは日本語のまま、上書きされない)。

例外として **ファイル不在分岐** では `FilesystemPreferencesRepository::load` 内で
`shared::detect_default_locale()` を呼び OS locale から ja / en を推定する。初回起動時のみ
sys_locale を読む形にして、テストや InMemory 経路では sys_locale を踏まないようにしている。
詳細は ADR-0042。

### 例外: テスト / CI

`InMemoryCharacterRepository` / `InMemoryPreferencesRepository` をテスト用に用意し、disk を一切触らずに Repository contract を検証する（→ ADR-0011）。

## Alternatives Considered

- **1 ファイルに統合（workspace pointer + preferences を 1 つの yml に）**: workspace 切替の度にテーマ等のユーザー設定が初期化される。あるいは「workspace の中に preferences を置く」案だと、別マシンで同じ workspace を開いた時にテーマが共有されてしまう。性質の違うものは別ファイルが素直。

- **環境変数で workspace pointer を渡す（`LOCAL_GW_WORKSPACE=...`）**: 書き戻しができない（FileDialog で選ばせて保存、という UX を作れない）。シェルから起動する都度設定するのも personal tool としては面倒。

- **OS のレジストリ / Windows Credential Manager**: クロスプラットフォームでない、単純な編集ができない、Git で履歴を残せない、と yml に対する利点が一切ない。

- **release build でも `current_dir`（CWD）を見る**: ユーザーがアプリを「どこから起動したか」で挙動が変わる。デスクトップアイコンからの起動と、エクスプローラからの起動で CWD が違うので不安定。`exe` の隣なら起動経路に依存しない。

- **debug build でも `exe` の隣を見る**: target/debug 配下に config を置く羽目になり、`cargo clean` で吹き飛ぶ。リポジトリ直下に置いて `.gitignore` する方が開発体験が良い。

- **preferences の不在時にも FileDialog を出す**: 起動するだけでダイアログが出るのは煩わしい。default で起動して、変えたくなったら Preferences モーダルから設定する方が無摩擦。

- **preferences が壊れたら起動失敗にする**: 一度壊れると手動修復するまで起動不能になる。fail-soft（default にフォールバック + warn ログ）の方が事故からの復帰が早い。

## Consequences

**得られたもの**

- workspace を切り替えてもテーマ・キーバインド等が保持される
- workspace を Git に push しても個人設定が混じらない
- 別マシンで同じ workspace を開いても、そのマシンのユーザー設定がそのまま効く
- preferences が破損しても起動できる。warn ログを見て修復するか、UI で書き直すか、ファイル削除すれば default に戻る
- debug と release で別の場所を見るので、開発中の experimental 設定を本番配布に紛れ込ませる事故が起きにくい

**支払うコスト**

- ファイルが 2 つあるので「設定がどこ？」と訊かれた時に 2 箇所を答える必要がある
- workspace pointer の保存先が debug / release で違うので、ドキュメントで明示する必要がある（`.claude/CLAUDE.md` に明記）
- preferences の `#[serde(default)]` 戦略は「フィールド追加には強いが、フィールドのリネーム / 削除には弱い」。リネーム時は旧名を `#[serde(alias)]` で残すか、マイグレーション関数を書く必要がある
- preferences の path は `dirs::config_dir()` 依存。テストでは `from_path` コンストラクタで tempdir を渡してバイパスする

**今後の拡張余地**

- 複数 workspace を頻繁に切り替えるなら、`local-gw.yml` を「最後に開いた workspace」+ 「履歴 5 件」に拡張して起動時にピッカーを出す UI が考えられる
- preferences を機能ごとに分割（`themes.yml` / `keybindings.yml` 等）する余地はあるが、現状サイズではオーバーキル
- workspace 単位の設定（例: そのプロジェクトでの table-of-contents 並び順）が欲しくなったら、`workspace/.local-gw/` 配下に第 3 層を足す。preferences とは別物として扱う

## 関連

- ADR-0011: Filesystem YAML を primary storage にする（`serde_saphyr` の選択を共有）
- ADR-0042: editor UI の i18n に rust-i18n を採用し locale を Preferences に持たせる（本 ADR の `locale` 補追）
- `shared/config.rs`: `local-gw.yml` のロード実装
- `entities/preference/api.rs`: `preferences.yml` の Repository 実装と fail-soft フォールバック
- `entities/preference/README.md`: `Preferences` フィールド一覧と直接 Signal 共有パターン
