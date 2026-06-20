# entities/keybinding — ショートカットキーのドメイン

エディタが認識する「ユーザー設定可能なコマンド (`Action`)」と、それに紐付く「キーバインド (`KeyBinding`)」を扱う slice。

## ファイル構成

| ファイル | Segment | 役割 |
|---|---|---|
| `model.rs` | model | `Action` enum、`KeyBindings` map (Default: Save=Ctrl+S、Undo=Ctrl+Z、Redo=Ctrl+Shift+Z) |
| `dispatch.rs` | api 補助 | `KeyboardActionRequest` (`seq` + `Option<Action>`)、`KeyboardActionDispatcher`、provider/hook |

`KeyBinding` 構造体本体は `shared/keybinding.rs` (汎用ヘルパー) にあり、ここでは Action 単位のドメインだけを扱う。

## なぜ `entities` の独立 slice なのか

`Action` は SpriteGroup / Animation / Character など複数の entity 横断のコマンド集合だが、それ自体が「ユーザー設定可能なコマンドの語彙」という独立ドメインを持つ。`shared` は外部依存ナシの汎用ヘルパー置き場なので合わない。`Preferences` も同様に独立 slice として扱われており、設計と整合する。

slice isolation: `keybinding` slice は `Action` 識別子と `KeyBindings` map のみを持ち、`character` 等を import しない (FSD のルールに従う)。

## ディスパッチの仕組み (counter bump 方式)

`Signal<KeyboardActionRequest { seq, action }>` を context で 1 つ共有する。

- グローバル listener (`RootShell`) が `KeyBinding::from_keyboard_event` で押下キーを取得し、`Preferences::key_bindings.resolve(&kb)` で **割り当てられた Action 群** を取り、`dispatcher.fire(action)` を順に呼んで `seq` を 1 進める
- 各画面は `use_keyboard_action(Action::Foo, handler)` hook で購読する。hook 内部の `use_effect` が `dispatcher.current()` を読んで、`seq` が前回と異なり `action == target` なら handler を呼ぶ
- 同 Action の連打や、同フレーム再発火でも `seq` の変化で確実にトリガーされる (Refresh トリガー / ADR-0004 と同じ発想)
- **複数エディタ共通のコマンドは 1 つの Action にまとめる**: 例えば `Save` / `Undo` / `Redo` は SpriteGroup / Animation / SoundGroup の 3 エディタ共通の単一 Action。`use_keyboard_action(Action::Save, ...)` は mount された Editor の Actions コンポーネントだけが register するので、active な editor 1 つだけが処理する（unmount で hook ごと消える）。設定 UI でも 1 行で管理できる

## 更新フロー

```
ユーザーが Ctrl+S 押下
  └── RootShell の onkeydown
        ├── KeyBinding::from_keyboard_event(&evt)
        ├── preferences.read().key_bindings.resolve(&kb) → vec![Save]
        ├── evt.prevent_default()  ← ブラウザ Ctrl+S 抑止
        └── for action in actions { dispatcher.fire(action) }
              └── Signal が更新される (seq++)
                    └── (リアクティブ伝播)
                          └── 現在 mount されている Editor の use_keyboard_action(Save) が発火
                                └── 例: SoundGroupEditor を開いていれば SoundGroupEditorActions の handler() 実行
```

## 画面側 API (`features/keybinding/ui/use_keyboard_action.rs`)

```rust
use_keyboard_action(Action::Save, move || on_save.call(()));
```

コンポーネントが unmount すると hook (= use_effect 購読) ごと自然に消えるので、listener 解除のクリーンアップは不要。これにより「画面がアクティブな間だけ Action を処理する」が成立する。

## 関連リファレンス

- `entities/preference/README.md`: 直接 Signal 共有パターン (Preferences 値の context 配布)
- ADR-0004 (Refresh トリガー): counter bump で再フェッチを促すパターンの原型
