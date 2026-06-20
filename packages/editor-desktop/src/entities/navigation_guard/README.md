# entities/navigation_guard — 未保存編集の離脱確認

「未保存の変更を抱えたまま画面を離れようとしたとき」に確認ダイアログを出すための、グローバル state を扱う slice。

設計判断の経緯は ADR-0008 を参照。ここは「どう使うか」のドキュメント。

## ファイル構成

| ファイル | Segment | 役割 |
|---|---|---|
| `model.rs` | model + api 補助 | `NavigationGuard` 構造体 + provider/hook |

## 状態

```rust
pub struct NavigationGuard {
    blocked: Signal<bool>,            // 編集中画面が「未保存あり」を宣言
    pending: Signal<Option<String>>,  // 確認待ちの遷移先 URL
}
```

`AppMain` で `use_navigation_guard_provider()` を 1 度だけ呼び、消費側は `use_navigation_guard()` で取得する。

## API と協調する 3 つのロール

| ロール | 呼ぶメソッド | 典型的な配置 |
|---|---|---|
| **編集画面** | `set_blocked(true/false)` | dirty 状態を `use_effect` で同期、`use_drop` で必ず解除 |
| **ナビ起点** (rail / breadcrumb / sidebar / Cancel) | `try_navigate(&nav, route)` | `Link` ではなく `<a>` / `<button>` の onclick から呼ぶ |
| **確認ダイアログ** (RootShell に常駐) | `pending()` で表示判定、`confirm(&nav)` / `cancel()` でクローズ | グローバル 1 箇所のみ |

## 典型的なフロー

```
ユーザーが編集画面で内容を変更
  └── SpriteGroupEditorActions の use_effect
        └── guard.set_blocked(true)

ユーザーが breadcrumb の "Character" をクリック
  └── <a onclick=...>
        └── guard.try_navigate(&nav, "/characters/Foo")
              ├── blocked == true → guard.pending = Some("/characters/Foo")
              └── (Signal 更新)
                    └── RootShell が confirm dialog を描画

ユーザーが「破棄して移動」をクリック
  └── guard.confirm(&nav)
        ├── nav.push(pending) で遷移
        ├── pending = None
        └── blocked = false

(編集画面が unmount される)
  └── use_drop で guard.set_blocked(false) ← 念押し
```

「やめる」を選んだ場合は `guard.cancel()` だけで `pending = None`。`blocked` はそのまま、編集を続けられる。

## 「未保存じゃない時」の挙動

`blocked == false` のときは `try_navigate` は単に `nav.push(route)` を呼ぶだけ。ダイアログは出ない。`Link` の代替として安全に使える。

## 新しいナビゲーションリンクを足すときの 3 ステップ

1. `Link { to: "..." }` を使わず、`<a class="cursor-pointer" onclick=...>` または `<button onclick=...>` を書く
2. コンポーネント先頭で `let mut guard = use_navigation_guard(); let nav = use_navigator();`
3. onclick で `guard.try_navigate(&nav, route_string)` を呼ぶ

```rust
// 例: characters_sidebar.rs の SidebarLink
let mut guard = use_navigation_guard();
let nav = use_navigator();
let route = format!("/characters/{character_name}");

rsx! {
    a {
        class: "px-3 py-2 rounded hover:bg-base-300 cursor-pointer",
        onclick: move |_| guard.try_navigate(&nav, route.clone()),
        "{character_name}"
    }
}
```

`Link` のデフォルト cursor pointer は失われるので `cursor-pointer` クラスを明示すること。

## 編集画面で dirty を同期するときの定型

```rust
use_effect(move || {
    let dirty = draft() != *baseline.read();
    guard.set_blocked(dirty);
});

use_drop(move || {
    guard.set_blocked(false);
});
```

`use_drop` は unmount 時に 1 度走るので、(a) 「破棄して移動」経由で抜けた後の余韻、(b) 例外的な遷移、いずれの場合でも `blocked` が立ちっぱなしにならない安全弁になる。

## 確認ダイアログ (`app/root_shell.rs`)

`RootShell` 末尾で `if guard.pending().is_some() { ... }` でグローバルに 1 つだけ描画。文言を変えるならここを編集する。

## 関連リファレンス

- ADR-0008: NavigationGuard for unsaved-edit confirmation (なぜこの設計か)
- ADR-0004: Refresh trigger as wrapping counter (Signal で global state を共有するもう 1 つの例)
- `.claude/docs/data-flow.md`: 「context-shared Signal」パターンの位置づけ
