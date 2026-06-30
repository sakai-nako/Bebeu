# ADR-0041: ユーザー側設定の永続化に Bevy 0.19 App Settings を採用

## Status

Accepted (2026-06-28、Issue #10 で試作 → 採用)

## Context

Bevy 0.19 で標準提供された `bevy::settings`
(`SettingsPlugin` + `#[derive(SettingsGroup)]` + `SaveSettingsDeferred` / `SaveSettingsSync`)
を engine 側で評価する (Issue #10、`bevy-settings = "0.19.0"` crate に内蔵)。

当 repo は ADR-0011 で「workspace 配下のゲームデータは YAML を SSoT」と決めているが、
これはあくまで *ゲームデータ* (project / level / character / sprite-group / animation) の話。
**ユーザー個人の起動間状態** (window 位置・サイズ・fullscreen、音量、将来は key binding) は
今まで整理されておらず、現状 engine 側に保存レイヤが無かった
(`bebeu-engine.yml` に `window: {width, height}` だけが developer/admin 配置として置かれていた)。

Bevy 0.19 で導入された App Settings は OS 規約に従う TOML 永続化機構
(`%LOCALAPPDATA%\<reverse-domain>\settings.toml` 等) を提供し、Bevy editor 本体も
これを使う想定 (リリースノート)。L1 ゲームデータ層 (YAML) と直交させて、L3 ユーザー設定層
として導入できる。

ADR-0037 (Bevy 0.19 移行) の「別 Issue で採用評価」項目を本 ADR で結論付ける。

## Decision

### 永続化レイヤを 3 層に整理する

| 層 | 対象 | 保存場所 | 形式 | 関連 ADR |
|---|---|---|---|---|
| **L1 ゲームデータ** | project / level / character / sprite-group / animation 等 | `workspace_dir/data/*.yml` | YAML (serde_saphyr) | ADR-0011 |
| **L2 engine 起動 config** | `workspace_dir` の pointer | `packages/engine/bebeu-engine.yml` | YAML | ADR-0016 (本 ADR で window を除外) |
| **L3 ユーザー設定** | window 状態 / master volume | OS 規約 directory (下記) | TOML (bevy-settings) | **本 ADR (0041)** |

L3 の保存先は `SettingsPlugin::new(app_name)` に渡す reverse-domain 名で決まる:

- Windows: `%LOCALAPPDATA%\com.hack-pleasantness.bebeu.engine\settings.toml`
- macOS:   `~/Library/Preferences/com.hack-pleasantness.bebeu.engine/settings.toml`
- Linux:   `~/.config/com.hack-pleasantness.bebeu.engine/settings.toml`

### 試作対象 (本 ADR で採用した SettingsGroup 2 つ)

- `WindowSettings` — `position: Option<IVec2>` / `size: Option<UVec2>` / `fullscreen: bool`。
  起動時に primary window へ反映、ユーザーが移動/リサイズ/fullscreen 化したら debounce
  500ms で保存、window close 時に同期保存 (`SaveSettingsSync::IfChanged`)。
- `AudioSettings` — `master_volume: f32` (0.0-1.0、default 1.0)。SE 発火時に `Volume::Linear` に
  multiply。Options scene の Master Volume 行で Left/Right による調整 + debounce 500ms 保存。

両 group は `settings.toml` 内の `[window]` / `[audio]` セクションに分かれて書かれる
(同一ファイル、別 group 名)。

### bebeu-engine.yml は `workspace_dir` 専用に縮小

旧 `window: {width, height}` セクションは廃止。初回起動時の解像度は engine 内 fallback
(`WINDOW_INTEGER_SCALE_FALLBACK = 3` × viewport) で確定する。配布物で初期サイズを変えたい
場合は engine crate のこの定数を変更する (= 再 build を伴う)。

### SettingsPlugin 配置順

`DefaultPlugins` の **後**、`UserSettingsPlugin` (glue) の **前** に add する。
`SettingsPlugin::build` 時点で `AppTypeRegistry` を読んで `ReflectSettingsGroup` を持つ型を
discover するため、`#[derive(Reflect)]` が auto-register される
`DefaultPlugins` 直後が正しい order (公式 `examples/window/persisting_window_settings.rs` に
従う)。

### Reflect の限定導入

App Settings の要件として `#[derive(Reflect)]` + `#[reflect(Resource, SettingsGroup, Default)]`
を engine crate に初導入する。**本 module (`shared/settings.rs`) でのみ使い、他用途には
広げない**。具体的には他の Resource / Component で Reflect 系 derive を増やさない
(将来 inspector や scene serialization が欲しくなった時点で別 ADR で再評価)。

### スコープ外 (将来別 Issue)

- **`input.yml` (gameplay key binding) の App Settings 移行** — `shared/input.rs` の
  設計と合わせて別 Issue で再評価。現状は ADR-0016 のまま `packages/engine/config/input.yml` に
  自前 YAML 保存。
- **pixel_perfect_config の resize 追従** — ユーザーが window を resize/fullscreen 化した
  際に中間 RT との整数倍関係が崩れる問題は別 Issue。本 ADR の責務は「state を保存して
  次起動で復元する」までで止める。
- **editor (Dioxus) 側 `preferences.yml` (ADR-0012) との統合** — FSD 規約「editor / engine は
  独立を維持」(CLAUDE.md) に従い、editor 側は引き続き ADR-0012 のレイアウトで運用する。
- **BGM 系の volume 分離 (`sfx_volume` / `bgm_volume`)** — BGM 実装がまだ無いので
  `master_volume` 1 つで SE のみ掛ける。BGM 導入時に分離を検討。

## Alternatives Considered

- **自前 YAML に揃える (ADR-0011 を L3 にも拡張)**: 形式は揃うが、OS 規約 directory の解決
  / cross-platform 対応 / debounce 保存 / atomic write (crash 耐性) を自前で書く必要があり、
  bevy-settings が標準で提供するものを捨てる損が大きい。また Bevy editor 本体が App Settings
  を使う将来を考えると、editor との UX 整合 (= 設定が両方の preferences directory に分散)
  を取りたい。

- **OS レジストリ / Windows Credential Manager**: cross-platform でない、編集しづらい、
  Git で履歴を残せない。L1 で YAML を選んだ理由 (ADR-0011) と同じ理由で却下。
  そもそも cross-platform を担保するなら bevy-settings の方が自然。

- **editor の `preferences.yml` (ADR-0012) を engine 側にも流用**: editor / engine 独立規約
  (CLAUDE.md) に反する。両者で同じ preferences.yml を共有すると、片方の format 変更が
  他方に波及する。L3 が cross-tool でなく cross-OS-user である以上、tool ごとに別の
  preferences directory を持つのが素直。

- **`bebeu-engine.yml` に window セクションを残し、App Settings は override で重ねる**:
  優先順位 (App Settings > yml > 内部 fallback) のロジックを保守する必要があり、admin が
  「yml に書いた値が反映されない」という挙動を見るとデバッグが面倒。「window はユーザー
  設定」と振り切って yml から外す方が責務分離が明確。配布者が初期サイズを変えたい場合は
  engine 内定数 (`WINDOW_INTEGER_SCALE_FALLBACK`) で対応する。

## Consequences

### 得られたもの

- OS 規約 directory への保存が標準実装で提供される (`dirs` crate / `bevy_platform` 経由)。
  配布物にユーザー設定が混入しない (cargo clean / 配布バイナリ削除でも残る)。
- `SettingsGroup` の宣言的定義 (`#[derive(SettingsGroup)]` + `#[settings_group(group = "...")]`)
  により、boilerplate (TOML I/O + serde 往復 + reflection apply) をゼロにできる。
- `SaveSettingsDeferred(Duration)` の debounce が標準提供。slider 連続変更で disk を叩かない。
- atomic write (tempfile → rename) で crash 中断時も TOML が破損しない (bevy-settings 内蔵)。
- L3 と L1/L2 が物理的に分離。workspace を Git push しても個人 window 位置が混入しない。

### 支払うコスト

- engine crate に **`Reflect` derive と関連 import** が増える (今まで 0 件)。compile 時間
  への影響は軽微 (1 module / 2 struct のみ) だが、依存方向としては `bevy_reflect` が
  binary size に響く可能性あり。配布バイナリ size は別途計測。
- `ExitCondition::DontExit` 化により、window close → `SaveSettingsSync::IfChanged` →
  `AppExit::Success` の経路を engine 側で明示的に書く必要がある。今後 `AppExit` を発行する
  他の経路 (例: in-game Quit メニュー) を足す場合は同じく save を呼ぶ責務がある。
- 永続化形式が `YAML (L1/L2) + TOML (L3)` の二重になる。AI / 人間の認知負荷が増える
  (どの設定がどの形式か判断する必要)。ADR 番号で位置付けを明示することで緩和。
- `SettingsPlugin` は **disk I/O を build 時に同期実行する** (= add 時点で TOML load)。
  smoke test では `SettingsPlugin` 自体は使わず、必要 Resource (`AudioSettings`) を
  `init_resource` で代替する (`packages/engine/tests/engine_smoke.rs`)。
- 初回起動 (App Settings TOML 不在) は内部 fallback サイズで window が開く。ユーザーが
  resize / fullscreen 化 → quit → 再起動の流れで、**一瞬 fallback サイズで開いてから設定値に
  resize される** チラつきが起こる可能性がある (公式 example も同じ挙動)。実用上問題なら
  別 Issue で `init_window_pos` を pre-window-creation phase に移す改修を検討。

### 今後の拡張余地

- key binding を `input.yml` から `KeyBindingsSettings` SettingsGroup に移行する (別 Issue)。
  Bevy `KeyCode` 列を `Reflect` で serialize できれば、本 ADR の枠組みでそのまま乗せ替え可能。
- BGM 実装時に `AudioSettings` を `master_volume` + `bgm_volume` + `sfx_volume` に拡張。
  既存 TOML との互換は `#[serde(default)]` 相当が Reflect 側でも効くため、フィールド追加に
  対しては前方互換。
- `WindowSettings.monitor_id` (どのモニタで開いたか) を追加する余地。multi-monitor 環境で
  fullscreen 復元時の挙動を細かく制御したくなった時に検討。
- editor 側を将来 App Settings に揃える場合は別 reverse-domain (例:
  `com.hack-pleasantness.bebeu.editor`) を使い、engine と保存先を分ける。

## 関連

- ADR-0011: Filesystem YAML を primary storage にする (L1 の根拠)
- ADR-0012: 設定ファイルを 2 層に分ける (editor 側の preferences.yml、本 ADR とは別 namespace)
- ADR-0016: engine config の hybrid 配置 (本 ADR で window 行を除外)
- ADR-0037: Bevy 0.19 移行で採否した新機能 (本 ADR でその「別 Issue 評価」を結論付け)
- `packages/engine/src/shared/settings.rs`: SettingsGroup 2 つの定義
- `packages/engine/src/app/entrypoint.rs`: SettingsPlugin wire + glue plugin (apply/track/save)
- `packages/engine/src/scenes/options.rs`: Master Volume 行 (Left/Right 調整 + SaveSettingsDeferred)
- `packages/engine/src/features/character/sound.rs`: master_volume gain 適用
- `bevy-settings = "0.19.0"`: 内部実装 (https://crates.io/crates/bevy-settings)
