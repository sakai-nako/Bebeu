# ADR-0042: editor UI の i18n に rust-i18n を採用し locale を Preferences に持たせる

## Status

Accepted (2026-06-29、Issue #30 Phase 1 で導入)

## Context

editor (`packages/editor-desktop`) の UI 文字列は当初すべて日本語ベタ書きで、Dioxus
の rsx 内に literal として 144 ファイル / 約 3580 件散在していた (PreferencesModal の
"閉じる"、各 toast、modal title、button label、tooltip 等)。

Public mirror 経由で editor を触る contributor / 国外プレイヤーが増える局面で
「英語化を後から差し込めない」設計のままだと、全 rsx に i18n key を撒き直す巨大 PR が
必要になる。早めに **i18n 機構と locale 切り替え経路を整える** ほうが手戻りが少ない、
というのが Issue #30 の動機。

なお engine 側 (HUD / overlay / pause menu 等) の UI も日本語ベタ書きだが、本 ADR の
scope 外 (CLAUDE.md「editor / engine は独立を維持」)。engine 側は別 Issue で扱う。

## Decision

### フレームワーク: `rust-i18n` 4.x を採用

| 候補 | 採否 | 理由 |
|---|---|---|
| **rust-i18n** | **採用** | macro (`t!()`) + YAML catalog の最小構成。editor 単独 / ja-en 2 種なら必要十分。catalog は compile-time embed なので runtime I/O 不要 |
| fluent-rs (Project Fluent) | 不採用 | ICU 系で複数形 / gender ロジックに強いが editor 文字列でこれが必要な箇所は無い。runtime catalog ロードや FTL syntax を学習する overhead に対して見返りが薄い |
| 自前 `HashMap<&str, &str>` | 不採用 | 翻訳数が少ないうちは最速だが、placeholder 置換 / fallback / 階層 key の機構を自前で書く羽目になる。Phase 2 以降のスケールで必ず乗り換えが必要になる |

### catalog 配置と format

- `packages/editor-desktop/i18n/{ja,en}.yml` (YAML、`rust_i18n::i18n!("i18n", fallback = "ja")` で読み込む)
- `lib.rs` の crate root で 1 度だけ macro を展開する (rust-i18n の制約)
- key は **階層化** (`preferences.close` / `app.startup_error_title` / `projects.index_empty_hint`)。rsx 内では `{t("preferences.close")}` の形で参照する
- fallback locale を `ja` に設定。en catalog に欠落キーがあれば ja の文字列が返る。両者欠落なら key 文字列がそのまま返る (rust-i18n の標準動作)

### Locale enum を `shared/i18n.rs` に置く

`Locale::{Ja, En}` を `shared/i18n.rs` に定義し、`entities/preference/model.rs` から
`pub use crate::shared::Locale;` で re-export する。

- `Preferences.locale: Locale` フィールドを追加 (`#[serde(default)]`、default は `Locale::Ja`)
- ADR-0012 の二段構成 (workspace pointer / user preferences) のうち `preferences.yml` 側に locale が乗る (= workspace を切り替えても言語設定は保持)

### OS locale 検出: 初回起動のみ走らせる

`shared/i18n.rs::detect_default_locale()` が `sys_locale::get_locale()` を読んで先頭 2 文字で `ja` / `en` / その他 (= 既定 ja) を判定する。

呼び出すのは **`FilesystemPreferencesRepository::load` がファイル不在を検出した時のみ**:

| ケース | locale の決まり方 |
|---|---|
| `preferences.yml` 不在 (初回起動) | `detect_default_locale()` (OS の言語に追従) |
| `preferences.yml` 存在 + `locale:` キーあり | YAML の値 |
| `preferences.yml` 存在 + `locale:` キー欠落 (既存ユーザー) | `Locale::default()` = `Ja` (既存ユーザーの体験を変えない) |

この設計により、英語環境のユーザーが editor を初めて起動すると en が出る一方、
過去に preferences.yml を作った日本語ユーザーは `locale` キー追加の影響を受けない。

### 反応性: `Signal<Preferences>` 経由で reactive 化

rust-i18n は thread-local で current locale を保持する (`set_locale` で更新)。
これを Dioxus signal と結びつけるため:

1. `app_root.rs` の `AppMain` で `use_effect(|| apply_locale(preferences.read().locale))` を回す。preferences signal が変わると `rust_i18n::set_locale` が呼ばれる
2. component 側は `entities/preference/provider.rs::use_t()` hook で reactive な翻訳関数を取得する。`use_t()` は内部で preferences signal を read するので、locale 変更で再レンダリングが走る
3. preferences signal を直接読まない経路 (`AppRoot` の Config エラー path 等) では `shared::translate()` を直呼び出しする。事前に `apply_locale(detect_default_locale())` を呼んで OS locale で表示する

### placeholder 付き翻訳

YAML 内で `%{name}` の placeholder を書き、`translate_args("...", &[("name", value)])` で
差し替える。`use_t_args()` も同様に reactive 版を提供。rust-i18n macro 側にも引数渡しの仕組み
(`t!("...", name = value)`) はあるが、runtime に値が決まるユースケース (error message 等)
では string 置換の方が boilerplate が少ないため自前 helper を用意した。

### 試作対象 (Phase 1)

本 ADR では「機構導入」の Phase 1 のみを scope に含める:

- 上記機構 (catalog / Locale / detect / apply / use_t)
- `PreferencesModal` の全 literal (title / close / theme legend / theme error)
- `ChangeLocaleSelect` (新規 feature、locale 切り替え dropdown)
- `pages/projects.rs` の `ProjectsIndex` 文字列 1 件 (page 層の end-to-end 確認用)
- `app_root.rs` の起動エラー文字列 / unexpected error 文字列

### スコープ外 (Phase 2 以降 / 別 Issue)

- 残り 144 ファイル / 約 3580 件の日本語 literal の置換 (Issue #30 Phase 2 で順次)
- toast / error message の体系的 i18n 化 (個別文字列は Phase 1 で触る範囲のみ)
- 動的フォーマット (`format!` 経由のメッセージ) の包括的整理
- `.claude/docs/` に「key 命名規約 / 新規文字列追加手順」の常設ドキュメント追加 (Phase 2 着手時に判断)
- engine 側 UI (HUD / overlay / pause menu) の i18n (別 Issue)
- ja / en 以外の locale (必要が出たタイミングで再評価)

## Alternatives Considered

- **i18n を `entities/i18n` という独立 slice に切る**: locale 1 値だけのために slice を増やす
  のは overkill。ADR-0012 の Preferences に相乗りすれば、provider / Repository / save 経路を
  そのまま流用できる。i18n が複雑化したら (catalog の hot reload、複数 catalog 切替等)
  別 slice 化を検討する。

- **`fluent-rs` 採用**: 複数形 / gender / 数値・日付の locale 形式が必要になったときの拡張余地は
  魅力的だが、editor の現状文字列でその機能が必要なものは無い。FTL syntax を導入する学習 cost
  と runtime catalog ロード経路の保守 cost が、得られる柔軟性を上回らない。engine 側で
  ICU を必要とする日が来たら engine 側で別途採用すれば良い (editor / engine 独立規約)。

- **Locale を `entities/preference/model.rs` に直接定義し shared に置かない**: shared/i18n.rs
  が detect / apply / translate を提供する以上、Locale 型もそこに置いて型と挙動を 1 module に
  集約する方が見通しが良い。entities/preference 側は re-export だけにする。

- **`use_t` hook を shared に置く**: shared から entities/preference (signal) を import すると
  FSD の依存方向 (shared → entities → features → widgets → pages) に逆行する。shared には
  純粋関数 (`translate` / `apply_locale`) だけ置き、Dioxus signal を読む hook は
  entities/preference/provider.rs に置く。

- **`apply_locale` を `change_locale.rs` の onchange で直接呼ぶ**: signal set と apply_locale
  の順序を間違える余地がある。`app_root.rs` の use_effect で signal を購読する経路に統一すれば、
  どこから locale が変わっても確実に rust_i18n に反映される。

- **OS locale 検出を `Preferences::default()` 内で呼ぶ**: テスト / InMemory repository が
  起動するたびに sys_locale を叩くことになり、テストの再現性が落ちる。検出は load 経路の
  「ファイル不在分岐」に限定し、`default()` は純粋関数として保つ。

- **placeholder 置換に rust_i18n macro 引数 (`t!("...", name = value)`) を使う**: macro 展開が
  compile-time なので動的 key (例えば locale 一覧の loop 内で key を組み立てる) に使えない。
  本 ADR では `translate_args` を自前で持ち、key も placeholder 名も runtime で渡せる形にした。

## Consequences

### 得られたもの

- 後続の i18n 化 (Phase 2 以降) は **同じ `use_t()` パターンを各 component に撒くだけ** で済む。
  新規 component を書く時のテンプレートが固定された
- locale 切り替えが UI runtime で完結する (再起動不要)。`apply_locale` 経由で `rust_i18n` の
  thread-local が更新され、Dioxus signal 経由で全 component が再レンダリングされる
- ja / en の両 catalog で key が欠落すると runtime に key 文字列がそのまま見える
  (`preferences.close` が UI に出る) ので、抜けが視認しやすい
- catalog ファイルが分離していて、翻訳作業を catalog 単位で並列化できる (LLM 補助で en の初版
  生成 → 人の校正、というフローが取りやすい)

### 支払うコスト

- editor crate に **`rust-i18n` (+ `rust-i18n-macro` / `rust-i18n-support`) と `sys-locale`**
  が追加された。`rust-i18n-support` は `walkdir` / `globwalk` / `serde_yaml` 等を持ち込むため
  compile 時間に若干影響する (~10 秒程度)。配布バイナリ size への影響は未計測
- `i18n!()` macro は **crate root (`lib.rs`) でしか呼べない**。shared/i18n.rs に置こうとすると
  `_rust_i18n_t` シンボルが解決できず compile error になる (本 Phase で踏んだ。修正: lib.rs に
  移動)
- 翻訳済み文字列の検索性が下がる (rsx 内 literal を grep して該当箇所を特定する操作が、
  「key を catalog で grep → component で key を grep」の二段になる)。key の階層化命名で
  ある程度緩和する
- `FilesystemPreferencesRepository::load` のファイル不在分岐で OS locale 検出が走るので、
  「default 比較」型のテストは locale フィールドを除外する必要がある (Phase 1 で 1 件修正済み)
- `rust_i18n::set_locale` は thread-local で global state。Dioxus desktop は main thread の
  event loop で動くので問題ないが、もし将来 worker thread で UI 描画を行う構成を検討するなら
  この前提を見直す必要がある

### 今後の拡張余地

- Phase 2 で残り 3580 件の literal を slice 単位で順次 i18n 化する (Issue #30 の Phase 2 案)
- 翻訳作業フロー (LLM 補助 → 人手校正) の運用を `.claude/docs/i18n.md` 等に固定する判断
  (Phase 2 着手時に判断)
- 数値 / 日付フォーマット (例: project の作成日表示) が必要になったら、`rust-i18n` の
  format / num-format 連携か、ICU 系 crate (`icu`) の限定導入を検討
- 翻訳 key を `t_keys::PREFERENCES_CLOSE` のような定数化する rust 流の type-safe 化 (rust-i18n
  4.x には `i18n_keys!` 系のサードパーティ macro がある)。文字列 typo を compile error にできる
  が、Phase 1 では runtime key で十分

## 関連

- ADR-0011: Filesystem YAML を primary storage にする (catalog format 選定の文化)
- ADR-0012: 設定ファイルの二段構成 (本 ADR で `Preferences.locale` 追加を補追)
- `packages/editor-desktop/src/shared/i18n.rs`: Locale / detect / apply / translate / translate_args
- `packages/editor-desktop/src/entities/preference/provider.rs`: use_t / use_t_args hook
- `packages/editor-desktop/i18n/ja.yml` / `i18n/en.yml`: 翻訳 catalog
- `packages/editor-desktop/src/lib.rs`: `rust_i18n::i18n!()` macro の crate root 展開
