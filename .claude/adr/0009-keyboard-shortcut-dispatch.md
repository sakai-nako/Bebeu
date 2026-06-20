# ADR-0009: Keyboard shortcut dispatch (Action enum + counter Signal)

## Status

Accepted

## Context

エディタにユーザーカスタマイズ可能なキーボードショートカットを導入する。最初の対象は SpriteGroup Editor の保存 (Ctrl+S) だが、将来「キャラクター作成」「テーマ切替」など複数アクションに広げたい。

要件:

1. **画面横断で発火**: フォーカスがどこにあっても (canvas, input 等) ショートカットが効く
2. **画面コンテキスト依存**: SaveSpriteGroup は SpriteGroup Editor 表示中だけ動作する。`/characters` を見ているときに Ctrl+S を押しても何も起きない
3. **カスタマイズ可能**: preferences.yml に保存し、Preferences モーダルで再キャプチャできる
4. **連打が漏れない**: 同じ Action を素早く連続発火しても各回が確実に画面側に届く
5. **async / 追加 runtime を入れない** (→ ADR-0002)

`Signal<Option<Action>>` をそのまま発火源にすると、Dioxus Signal は同じ値での `set` を no-op にする (メモ化) ため、同じ Action の連打が 2 回目以降漏れる。これは ADR-0004 と同じ問題。

## Decision

3 層構成にする:

| レイヤー | 配置 | 役割 |
|---|---|---|
| 技術ヘルパー | `shared/keybinding.rs` | `KeyBinding` / `KeyModifiers` / parse / Display / `from_keyboard_event` |
| ドメイン | `entities/keybinding/` | `Action` enum (closed set), `KeyBindings(HashMap<Action, KeyBinding>)`, dispatcher Signal |
| 配線 | `features/keybinding/ui/` | グローバル listener コンポーネント / `use_keyboard_action` hook / Preferences 編集 UI |

### Dispatcher

ADR-0004 と同じ wrapping counter を Action 通知に流用する:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyboardActionRequest {
    pub seq: u64,                  // wrapping counter
    pub action: Option<Action>,    // 初期値は None
}
```

グローバル listener (`RootShell` の outer `div` の `onkeydown`) が押下キーを `KeyBinding::from_keyboard_event` で抽出し、`Preferences::key_bindings.resolve(&kb)` で Action に解決して `dispatcher.fire(action)` を呼ぶ。`fire` は seq を `wrapping_add(1)` で進めるので、同 Action 連打でも変化検知が確実に効く。

### 画面側 hook

```rust
use_keyboard_action(Action::SaveSpriteGroup, move || on_save.call(()));
```

内部は `use_effect` で dispatcher を購読し、`seq` が更新されかつ `action == target` のときに handler を呼ぶ。`use_signal` で `last_seen_seq` を保持して初期 seq=0 の誤発火を防ぎつつ連打にも追従する。

コンポーネントが unmount すれば `use_effect` 購読ごと消えるので、画面コンテキスト依存性が自然に成立する (要件 2)。

### Preferences との接続

`Preferences` に `key_bindings: KeyBindings` フィールドを追加 (`#[serde(default)]` で旧 yml 後方互換)。YAML には `KeyBinding` を `"ctrl+s"` 形式の文字列 1 行で書く (`#[serde(try_from = "String", into = "String")]`)。

## Alternatives Considered

- **`Signal<bool>` トグル / `Signal<Option<Action>>` 単独**: 同じ値での set が no-op になり、同 Action 連打が漏れる (ADR-0004 と同じ理由)。
- **画面ごとに `onkeydown` を仕込む**: フォーカスが当該要素にないと拾えない。canvas を触っているとき / input にフォーカス中など、グローバル shortcut の意図に反する。
- **Channel / event bus (`tokio::sync::broadcast` 等)**: async / 追加 runtime 依存 (ADR-0002 違反)。
- **キーバインドを直接マッチさせる (Action enum 不在)**: preferences でリマップできない。「Ctrl+S → SaveSpriteGroup」の対応はランタイムで決まる必要がある。
- **`Action` を非 closed set (string-based command id)**: タイプセーフ性とコード補完を失い、`Action::ALL` ループによる Preferences UI 自動生成もできなくなる。
- **`KeyBinding` を構造体ネストで YAML に書く**: `{ modifiers: { ctrl: true }, key: "s" }` は冗長で人間編集しづらい。`"ctrl+s"` 1 行のほうが preferences.yml を直接いじりたいケースで楽。

## Consequences

**得られたもの**

- 新 Action 追加は **3 ステップ**で完結:
  1. `Action` enum に variant 追加 (+ `ALL` / `label` / `id`)
  2. `Default for KeyBindings` にデフォルトキーを 1 行追加
  3. 該当画面で `use_keyboard_action(Action::NewOne, callback)` を 1 行追加
- Preferences UI は `Action::ALL` をループするだけで自動生成 (重複検出 / 再キャプチャ含む)
- 同フレーム連打や、同じ Action を別契機で複数回発火しても確実に effect が再実行される
- 画面の mount/unmount に listener 解除が自動で追従するので、画面横断のショートカット衝突を心配しなくていい

**支払うコスト**

- counter+Action タプルというやや non-trivial なシグネチャを覚える必要がある (ADR-0004 を読めば同じ思想)
- グローバル `onkeydown` を `RootShell` の outer `div` に仕込むため、`tabindex="-1"` + `autofocus` でフォーカスを掴む小細工が要る
- `KeyBinding::from_keyboard_event` は keyboard_types 依存なので、Dioxus のバージョンを上げるときに API 互換性をチェックする必要がある

**今後の拡張余地**

- macOS の Cmd 対応: `KeyModifiers::meta` フィールドが予約済み。`from_keyboard_event` に `#[cfg(target_os = "macos")]` 分岐を 1 箇所追加するだけで Cmd を Ctrl にマップ等の挙動を切替えられる
- コマンドパレット (Ctrl+Shift+P 風) への流用: `Action::ALL` と `Action::label` がそのまま検索対象になる
- 画面コンテキストの細分化: 現状は「画面 mount = handler 有効」で十分だが、同画面内で複数モードがある場合は `use_keyboard_action(Action::Foo, |if cond { run() })` のように handler 側で gating すれば対応可能
