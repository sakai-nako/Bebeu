# ADR-0008: NavigationGuard for unsaved-edit confirmation

## Status

Accepted

## Context

SpriteGroup Editor のような編集画面では「未保存の変更を抱えたまま画面を離れる」経路が複数ある:

1. 編集画面内の Cancel ボタン (詳細ページに戻る)
2. 画面内 breadcrumb のリンク (Character 詳細 / SpriteGroup 詳細)
3. レイアウトの左 rail (Characters アイコン)
4. レイアウト常駐の Characters サイドバー (各キャラクターへのリンク)

最初は (1) だけにローカル confirm ダイアログを置いていたが、(2)〜(4) は無音で離脱できてしまい、編集が失われる事故が起きうる。要件:

- 編集中以外 (dirty=false) のときは確認なしで素直に遷移する
- 編集中 (dirty=true) のときは離脱経路を問わず同じ確認ダイアログを出す
- 編集画面側はナビ要素の存在 / 場所を知らずに「未保存」だけ宣言する
- async / 追加 runtime を増やさない (→ ADR-0002)

`Link` コンポーネントは Dioxus 内部で直接 navigate するため、onclick で `prevent_default` を呼んでも止められる保証がない (実装依存)。`Link` をそのまま使い続ける戦略では確実性が足りない。

## Decision

`NavigationGuard` をグローバル context に置き、すべてのナビ起点をこの guard 経由に統一する。

### State

```rust
#[derive(Clone, Copy)]
pub struct NavigationGuard {
    blocked: Signal<bool>,             // 編集中画面が「未保存あり」を宣言
    pending: Signal<Option<String>>,   // 確認待ちの遷移先 URL
}
```

`AppMain` で 1 度だけ `use_navigation_guard_provider()` で context に置く。

### 三方向の協調

1. **編集画面 (例: `SpriteGroupEditorActions`)**
   - `use_effect(move || guard.set_blocked(is_dirty))` で dirty を guard に同期
   - `use_drop(move || guard.set_blocked(false))` で unmount 時に必ず解除

2. **ナビ起点 (Link 相当のすべて)**
   - 生の `Link { to: ... }` を使わず、`<a class="cursor-pointer" onclick=...>` または `<button onclick=...>` に置換
   - onclick 内で `guard.try_navigate(&nav, route)` を呼ぶ
   - `try_navigate` は blocked なら `pending` に積む、そうでなければ即 `nav.push`

3. **確認ダイアログ (`RootShell` に 1 つだけ)**
   - `guard.pending().is_some()` のとき描画
   - 「破棄して移動」: `guard.confirm(&nav)` → `pending` を navigate して `blocked=false` に戻す
   - 「やめる」: `guard.cancel()` → `pending` を消すだけ

実装は `entities/navigation_guard/model.rs`、消費側は `app/root_shell.rs` (rail + dialog)、`widgets/character/ui/sprite_group_editor.rs` (breadcrumb)、`widgets/character/ui/characters_sidebar.rs` (sidebar)、`features/character/ui/edit_sprite_group.rs` (Cancel + dirty 同期)。

## Alternatives Considered

- **Dioxus Router の `RoutingCallback`**: ルータ config に登録するグローバルフック。コンポーネント単位の dirty 状態と疎結合で、結局 component-local dirty を runtime に伝える経路が別途必要。callback を間に挟む価値が薄い。
- **Per-screen guard (編集画面が自分で confirm を持つ)**: 画面外 (sidebar / 左 rail) のリンク click を捕まえられない。最初の実装がこの形で、ユーザー視点で「サイドバーから無音で抜けられる」問題が顕在化した。
- **`Link` を維持して onclick で prevent_default**: Dioxus `Link` の内部実装が onclick より先に navigate する可能性があり、確実に止められる保証がない。コンポーネント置換のほうが安全。
- **未保存中はナビゲーションボタンを disable**: ユーザーから「破棄して移動」の選択肢を奪うため UX が劣る。
- **Browser `beforeunload`**: desktop webview では発火しない / 「保留して再開」の semantics がない。
- **bool ではなく `Signal<u32>` で dirty 件数を持つ**: 複数編集画面が同時に dirty になるケースを考えると一見便利だが、現状 single editor 前提なので overkill。`bool + use_drop` で十分。

## Consequences

**得られたもの**

- 離脱経路がいくつ増えても、`Link` を生で使わずに `try_navigate` に流せばダイアログが自動で出る
- 編集画面側はナビ要素の場所を知る必要がない。`set_blocked` するだけ
- 確認ダイアログは 1 箇所 (`RootShell`) に集約。スタイル変更も 1 箇所で済む
- ローカル `show_cancel_confirm` Signal とローカルダイアログを廃止できた (Cancel ボタンも `try_navigate` 経由に統一)

**支払うコスト**

- 新しいナビゲーション要素を増やすときは「`Link` を使わない」ルールを守る必要がある (人間規約)
- `Link` のデフォルトスタイル (cursor pointer 等) は失われるので、`<a>` 置換時は `cursor-pointer` クラスを明示
- dirty 同期は `use_effect` + `use_drop` のセットを編集画面ごとに書く必要がある
- グローバル `Navigator` を `&Navigator` で受け回す ergonomics: `guard.try_navigate(&nav, route)` の引数 1 個増えるのは許容範囲

**今後の拡張余地**

- multi-document 編集 (タブ複数) になったら `blocked: Signal<u32>` (件数) に拡張する。あるいは「どの editor が dirty か」の id を `blocked: Signal<HashMap<EditorId, ()>>` 化する
- 「破棄して移動」「やめる」以外に「保存して移動」を選択肢に加える場合は、`pending` と並列に「保存ハンドラ」を保持する仕組み (Action 化) が必要
- 外部 URL や OS レベルのウィンドウ閉じ操作は現状未捕捉。Wry の close handler 経由で `guard.is_blocked()` を見て確認を出す拡張は可能
