# entities/preference — ユーザー設定（テーマ等）

ユーザーの環境設定を扱う slice。Character 集約と同レイヤー（entities）だが、データ性質が大きく違うため別の slice にしている。

## `Preferences` のフィールド

| フィールド | 型 | 由来 | 備考 |
|---|---|---|---|
| `theme` | `Theme` (Emerald / Dark) | `model.rs` | daisyUI のテーマと対応。`app_root.rs` で `data-theme` 属性に反映 |
| `locale` | `Locale` (Ja / En) | `shared/i18n.rs` (re-export) | editor UI の表示言語。`app_root.rs` で `rust_i18n::set_locale` に反映 (→ ADR-0042) |
| `view_controls` | `ViewControlBindings` | `shared/view_controls.rs` | パン用マウスボタン / ホイール反転。Zoom 倍率は固定リスト (`ZOOM_LEVELS`) を階段する方式でユーザー設定にしない (sprite が subpixel 位置に着地して frame 切替で半 px ずれて見えるのを防ぐため) |
| `key_bindings` | `KeyBindings` | `entities/keybinding/` | Action → KeyBinding マップ。グローバル listener と Preferences 編集 UI が参照 (→ ADR-0009) |
| `sprite_group_history_capacity` | `u32` | `model.rs` | SpriteGroup Editor の Undo 履歴上限ステップ数（デフォルト 50） |
| `animation_history_capacity` | `u32` | `model.rs` | Animation Editor の Undo 履歴上限ステップ数（デフォルト 50） |

すべて `#[serde(default)]`。古い preferences.yml にフィールドが欠けていても `Default::default()` で自動補完される (fail-soft 既存方針)。

## ファイル構成

| ファイル | Segment | 役割 |
|---|---|---|
| `model.rs` | model | `Preferences` / `Theme` (Locale は `shared::Locale` の re-export) |
| `api.rs` | api | `PreferencesRepository` trait + InMemory + Filesystem (ファイル不在時のみ OS locale 検出) |
| `provider.rs` | api 補助 | `Signal<Preferences>` を context で配布する hook + reactive 翻訳 `use_t` / `use_t_args` |

## Character 集約との設計上の違い

| 観点 | Character | Preferences |
|---|---|---|
| インスタンス数 | N（ユーザーが作成） | 1（global singleton） |
| ストレージ | workspace/data/ 配下（プロジェクトに紐づく） | OS の config_dir 配下（ユーザーに紐づく） |
| UI 反映方法 | Refresh トリガー（カウンター bump → use_effect で再フェッチ） | **`Signal<Preferences>` を context 直接共有** |
| ロード失敗時 | エラー表示 | InMemory にフォールバック（→ app/README.md） |

## 「直接 Signal 共有」を選んだ理由

Refresh トリガーパターン（ADR-0004）はリスト系の更新を「再フェッチ」で扱う仕組み。Preferences は **値が 1 つしかない** ので:

- 再フェッチ不要（メモリ上の Signal 値が即 source of truth）
- 全消費者を同じ Signal で同期できる
- 変更も `repo.save() → signal.set()` の 2 ステップでよい

```rust
pub fn use_preferences_provider(initial: Preferences) -> Signal<Preferences> {
    use_context_provider(|| Signal::new(initial))
}

pub fn use_preferences() -> Signal<Preferences> {
    use_context::<Signal<Preferences>>()
}
```

## 更新フロー

```
ユーザーがテーマ切替（features/preference/ui/change_theme.rs）
  └── <select onchange>
        ├── repo.save(&new_prefs)        ← 1. disk に書く（失敗したら止まる）
        └── signal.set(new_prefs)         ← 2. メモリの Signal 更新
              └── (リアクティブ伝播)
                    ├── app_root の use_effect
                    │     └── document::eval で <html data-theme=...> 更新
                    └── 他の購読側も再レンダ
```

**「save 成功確認 → set」の順序が重要**。逆だとメモリと disk が乖離する。

## ストレージ位置

`{config_dir}/local-game-editor/preferences.yml`。`config_dir` は `dirs::config_dir()` の OS 標準パス:

| OS | パス |
|---|---|
| Windows | `%APPDATA%\local-game-editor\preferences.yml` |
| macOS | `~/Library/Application Support/local-game-editor/preferences.yml` |
| Linux | `~/.config/local-game-editor/preferences.yml` |

workspace 配下ではない（プロジェクトを切り替えてもユーザー設定は引き継がれる）。

## ロード失敗時のフォールバック

`FilesystemPreferencesRepository::new()` が失敗した場合（権限エラー等）、`AppMain` は `InMemoryPreferencesRepository` にフォールバックする:

```rust
let preferences_repo: Arc<dyn PreferencesRepository> =
    match FilesystemPreferencesRepository::new() {
        Ok(repo) => Arc::new(repo),
        Err(e) => {
            tracing::warn!("Preferences ストレージ初期化失敗: {} (InMemory)", e);
            Arc::new(InMemoryPreferencesRepository::new())
        }
    };
```

設定保存ができないだけでアプリ起動を止めるべきではない、という判断。

YAML が破損していて `load()` が失敗したケースも `Preferences::default()` を採用する（テストあり: `filesystem_load_returns_default_when_yaml_is_broken`）。

## 関連リファレンス

- `.claude/docs/data-flow.md`: 「Preferences Signal（直接共有パターン）」節
