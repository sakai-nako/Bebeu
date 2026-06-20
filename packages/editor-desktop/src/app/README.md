# app — ブートストラップ、DI 配線、ルーティング

editor の最上位レイヤー。誰も import しない（`pub use entrypoint::entrypoint` だけが Public API）。アプリ起動から最初のページが描画されるまでの配線を持つ。

## ファイル構成

| ファイル | 役割 |
|---|---|
| `entrypoint.rs` | `dx::launch_desktop` の呼び口。ロガー初期化 + Window 設定 |
| `app_root.rs` | `AppRoot`（Config ロード）→ `AppMain`（context 提供） |
| `routes.rs` | `Routes` enum（Routable derive）。ルート定義のシングルソース |
| `root_shell.rs` | アプリ全体の枠（左 rail + Outlet）。Preferences モーダルもここ |
| `characters_layout.rs` | Character master-detail shell。サイドバー + 詳細 Outlet |
| `asset_handler.rs` | `/workspace-asset/...` URL の resolver。ADR-0005 |

## 起動シーケンス

```
entrypoint()
  └── dx::launch_desktop(AppRoot)
        └── AppRoot
              └── Config::load()                    ← packages/editor-desktop/bebeu-editor.yml
                    └── AppMain { workspace_dir }
                          ├── use_workspace_asset_handler(workspace_dir)
                          ├── use_context_provider(Arc<dyn CharacterRepository>)
                          ├── use_characters_refresh_provider()
                          ├── use_context_provider(Arc<dyn PreferencesRepository>)
                          ├── use_preferences_provider(initial_prefs)
                          ├── use_effect: data-theme 属性の同期
                          └── ErrorBoundary > Router::<Routes>
                                  └── RootShell > CharactersLayout > Outlet
```

`AppRoot` で Config ロードに失敗するとエラー画面を直接描画し、`AppMain` には進まない（Provider が存在しない状態で children が走らないように分けている）。

## DI 提供物（Dioxus context）

`AppMain` で 1 度だけ provide する:

| Context | Type | 提供方法 |
|---|---|---|
| Character Repository | `Arc<dyn CharacterRepository>` | `FilesystemCharacterRepository::new(workspace_dir)` |
| Characters Refresh | `CharactersRefreshTrigger` | `use_characters_refresh_provider()`（→ ADR-0004） |
| Preferences Repository | `Arc<dyn PreferencesRepository>` | `FilesystemPreferencesRepository::new()` 失敗時は `InMemoryPreferencesRepository` にフォールバック |
| Preferences Signal | `Signal<Preferences>` | `use_preferences_provider(initial)`（→ entities/preference の README） |

Preferences 初期化で **Filesystem 失敗時に InMemory にフォールバック** する設計は、設定ファイル破損で起動不能にしないため。`tracing::warn!` でログには残す。

## ルーティング: 2 段ネスト layout

```rust
#[layout(RootShell)]              // アプリ全体の shell（左 rail）
    #[layout(CharactersLayout)]   // Characters セクションの shell（サイドバー）
        #[redirect("/", || Routes::CharactersIndex {})]
        #[route("/characters")]                                 CharactersIndex {}
        #[route("/characters/:name")]                            CharacterDetailPage
        #[route("/characters/:name/sprite-groups/:group")]       CharacterSpriteGroupPage
        #[route("/characters/:name/sprite-groups/:group/edit")]  SpriteGroupEditorPage
        #[route("/characters/:name/animations/:anim")]           CharacterAnimationPage
```

責務分離:

- **`RootShell`**: アプリ全体に常駐する UI。左 rail（Characters / Settings）。Preferences モーダル。`Outlet::<Routes>` で内側の layout を流す。
- **`CharactersLayout`**: Characters セクション専用の master-detail shell。`use_characters_refresh()` を購読し、`bump()` のたびにサイドバーが更新される。`active_name` を Routes から導出してハイライト。

新セクション（例えば Settings ページ群）を追加するときは:

1. `Routes` に layout 行と route 行を増やす
2. その layout component を `app/` 直下に置く（`xxx_layout.rs`）
3. ページ本体は `pages/` 側に追加

`pages/` は thin controller として URL パラメータから entity を解決して widget に渡すだけ。layout に依存ロジックを持ち込まない。

## 設計上の注意

- `AppMain` の context provide は **順序依存**: Preferences Repository → Preferences Signal の順番で provide しないと `use_preferences_provider` が初期値計算に使う Repository を見つけられない。
- `use_workspace_asset_handler` は Provider ではないが、`AppMain` 直下で 1 度だけ呼ぶ必要がある（Dioxus の `use_*` ライフサイクルに従う）。
- `ErrorBoundary` は `Router::<Routes>` の外側に置く。Router 内のページが panic / Error を返した時のフォールバック。

## 関連 ADR

- ADR-0001: FSD 採用（このレイヤーがその最上位）
- ADR-0004: Refresh トリガー（context 提供箇所）
- ADR-0005: WebView アセットハンドラ
