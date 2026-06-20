# Data Flow Architecture

editor のデータフローと UI 状態管理の現状を記述する。

## Overview

- **async/await を使わない**: editor では `async fn` / `.await` を書かない。Repository も event handler も `use_effect` も同期で完結させる
- **同期 Repository**: trait のメソッドはすべて同期。`async` を付けない
- **UI 状態は Dioxus Signal**: ページ単位 / context 経由で共有
- **Rust 側のキャッシュ層は無し**: Repository も `use_asset_handler` のクロージャも呼び出しごとに直接ファイル I/O。必要性が出てきたら導入する。WebView 側には HTTP キャッシュがある（後述）
- **mutation 後の再フェッチは Refresh トリガー**: `Signal<u64>` のカウンターを bump してリアクティブに `use_effect` を再実行させる
- **重い処理は `std::thread::spawn` でオフロード**: 現状は同期で I/O を直接呼んでいる。今後 UI が引っかかるほど重い処理が出てきた場合は、`spawn(async ...)` ではなく `std::thread::spawn` で別 OS スレッドに逃がす

### async ではなく `std::thread::spawn` を選ぶ理由

Dioxus の async runtime はシングルスレッドなので、`spawn(async { heavy_io() })` の中で同期 I/O をブロックすると **UI スレッドごと止まる**。素直に OS スレッドに逃がすほうが意図が明確で、`tokio::task::spawn_blocking` のような追加依存も発生しない。Signal は `Send` なので、別スレッドで処理した結果を Signal に書き戻すこともできる。

参考: <https://dioxuslabs.com/learn/0.7/essentials/basics/async/#concurrency-vs-parallelism>

---

## Repository Layer

各エンティティ集約は `entities/{entity}/api.rs` に trait + 2 つの実装を持つ:

- `{Entity}Repository`: 同期 trait（`Send + Sync` 必須）
- `InMemory{Entity}Repository`: テスト用、`RwLock<HashMap<String, T>>` 等で実装
- `Filesystem{Entity}Repository`: 本番用、YAML を読み書き

集約の実装例:

| Entity | ストレージ位置 | 主な責務 |
|---|---|---|
| `Character` | `{workspace_dir}/data/characters/` | キャラクターと子集約の YAML |
| `Preferences` | `{config_dir}/local-game-editor/preferences.yml` | ユーザー設定（テーマ等）。`config_dir` は OS 標準（Windows: `%APPDATA%`、macOS: `~/Library/Application Support/`、Linux: `~/.config/`） |

DI は `Arc<dyn {Entity}Repository>` を Dioxus context で配布。app/app_root.rs で provide:

```rust
use_context_provider(move || {
    Arc::new(FilesystemCharacterRepository::new(workspace_dir))
        as Arc<dyn CharacterRepository>
});
```

消費側（pages, widgets, features）は `use_context::<Arc<dyn CharacterRepository>>()` で取得。

### list と get の非対称性

集約に子集約が含まれる場合（例: Character は `sprite_groups: Vec<SpriteGroup>` を持つ）、`list()` は metadata だけ返し、`get(name)` は子集約も完全にロードする。一覧表示で全体を読まず、軽量化する。

### 更新粒度（Filesystem 実装の責務範囲）

`CharacterRepository` の write 系メソッドは触る対象が異なる:

| メソッド | `{name}.yml` | `sprite-groups/*.yml` | `sprite-groups/{group}/sprites/*` (実バイナリ) | 主な用途 |
|---|:-:|:-:|:-:|---|
| `create` | 新規作成 | 全件書き込み | 触らない | Character 新規作成 |
| `update` | 上書き | 差分同期（不要な yml は削除） | 触らない | sprite_groups を含む batch 編集 |
| `update_metadata` | 上書き | **触らない** | 触らない | inline 編集（HP 等の単項目変更） |
| `rename` | move + name 書換 | （ディレクトリごと move） | （ディレクトリごと move） | Character の改名 |
| `delete` | 削除 | （ディレクトリごと削除） | （ディレクトリごと削除） | Character の削除 |
| `import_sprite_image` | 触らない | 触らない | コピー作成 | サムネイル等の画像取り込み |
| `delete_sprite_image` | 触らない | 触らない | 単一ファイル削除 | `import_sprite_image` のロールバック |

inline 編集で `update` を呼ぶと sprite-groups/*.yml が全部書き直されて mtime が変わるので、軽量更新が必要なら `update_metadata` を使う。

---

## UI 状態管理

### Per-component Signal

ページや widget が自分の状態を `use_signal(...)` で持つ。

```rust
let mut character = use_signal(|| None::<Character>);
let mut error = use_signal(|| None::<String>);
```

### Cross-component Signal

レイアウト (`app/characters_layout.rs`) が共有可能なリストを `use_signal` で持ち、context で配布する場合がある。今のところ Character 一覧は layout 内に閉じていて、widget には `Vec<Character>` を prop で渡す形。

### Preferences Signal（直接共有パターン）

ユーザー設定 (`Preferences`) はリストではなく単一値なので、Refresh トリガーを使わず **`Signal<Preferences>` を直接 context で配布**する。`entities/preference/provider.rs`:

```rust
pub fn use_preferences_provider(initial: Preferences) -> Signal<Preferences> {
    use_context_provider(|| Signal::new(initial))
}

pub fn use_preferences() -> Signal<Preferences> {
    use_context::<Signal<Preferences>>()
}
```

**フロー**:

1. 起動時: `app/app_root.rs` で `repo.load()` → `use_preferences_provider(initial)` で Signal を context に配置
2. 適用: 同じく `app_root` の `use_effect` で Signal を購読し、`document::eval` で `data-theme` 属性を更新
3. 変更: `features/preference/ui/change_theme.rs` の `<select onchange>` で **先に `repo.save()` 成功を確認 → 次に `signal.set(new)`** の順で更新（disk と memory の乖離を防ぐ）
4. リアクティブ: Signal 更新で `use_effect` が再実行され、テーマが live で切り替わる

### NavigationGuard（離脱確認 Signal）

「未保存の変更を抱えたまま画面を離れようとした時」に確認ダイアログを出すためのグローバル state。Preferences と同じく context-shared Signal だが、書き手 (編集画面) と読み手 (各 Link / 確認ダイアログ) が分かれている点がポイント。

```rust
pub struct NavigationGuard {
    blocked: Signal<bool>,            // 編集中画面が「未保存あり」を宣言
    pending: Signal<Option<String>>,  // 確認待ちの遷移先 URL
}
```

**フロー**:

1. 編集画面が `use_effect` で `is_dirty` を `guard.set_blocked(...)` に同期し、`use_drop` で unmount 時に必ず解除する
2. すべてのナビ起点 (rail / breadcrumb / sidebar / Cancel) は `Link` ではなく `<a>` / `<button>` の onclick で `guard.try_navigate(&nav, route)` を呼ぶ
3. blocked のとき `pending` に積む、そうでなければ即 `nav.push`
4. `RootShell` が `guard.pending().is_some()` のときに確認ダイアログを描画。「破棄して移動」で `confirm(&nav)`、「やめる」で `cancel()`

詳細は ADR-0008 と `entities/navigation_guard/README.md`。

### KeyboardActionDispatcher（Action 通知）

ショートカットキー押下を Action に解決して画面側 hook に届けるグローバル state。Refresh トリガーと同じ wrapping counter を Action 通知に流用している。

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyboardActionRequest {
    pub seq: u64,
    pub action: Option<Action>,
}
```

`RootShell` の outer `div` の `onkeydown` がキー押下を Action に解決して `dispatcher.fire(action)` を呼ぶと `seq` が wrapping_add で進む。画面側は `use_keyboard_action(Action::Foo, handler)` で当該 Action を購読する。詳細は ADR-0009 と `entities/keybinding/README.md`。

### Refresh トリガー

mutation 後にレイアウトのリストを再フェッチさせるための仕組み。entities 層の [refresh.rs](../../packages/editor/src/entities/character/refresh.rs):

```rust
#[derive(Clone, Copy)]
pub struct CharactersRefreshTrigger(Signal<u64>);

impl CharactersRefreshTrigger {
    pub fn subscribe(&self) -> u64 {
        self.0.read().to_owned()
    }
    pub fn bump(&mut self) {
        let next = self.0.read().wrapping_add(1);
        self.0.set(next);
    }
}
```

**カウンターを使う理由**: Dioxus Signal は値が変わった時だけ購読者に通知する。bool toggle だと同フレーム内の複数 set が coalesce されて初期値に戻る可能性があるが、wrapping カウンターは必ず値が変化するため確実に effect を再実行できる。

---

## Data Flow Diagrams

### Read（一覧表示）

```
CharactersLayout
  ├── use_signal(|| Vec<Character>::new)
  ├── use_signal(|| None::<String>)            ← error
  └── use_effect (subscribes to refresh trigger)
        ↓ refresh.subscribe() で購読
        ↓ trigger 値が変わったら再実行
       CharacterRepository::list()
        ↓ std::fs::read_dir + yaml::from_str
       Filesystem (data/characters/*.yml)
        ↓
       characters.set(list)
        ↓ Signal 更新
       CharactersSidebar 再レンダリング
```

### Write（mutation）

```
features/character/create_character.rs (CreateCharacterButton)
  └── form submit
        ↓ button onclick
       CharacterRepository::create(&new_char)
        ↓ std::fs::write
       Filesystem に新規 yml + sprite-groups/ ディレクトリ
        ↓ on success
       refresh.bump()             ← Signal<u64> 値変化
        ↓ Dioxus reactivity
       CharactersLayout の use_effect 再実行
        ↓
       CharacterRepository::list() 再読込
        ↓
       characters Signal 更新 → サイドバーに新規項目が出る
```

Delete も同じパターン。Delete の場合は `CharacterRepository::delete(name)` の後に `bump()` + ナビゲータで一覧へ戻る。

---

## アセット配信と WebView キャッシュ

`<img src="/workspace-asset/...">` は [app/asset_handler.rs](../../packages/editor/src/app/asset_handler.rs) の `use_asset_handler` で受け、`workspace_dir` 配下の実ファイルを `std::fs::read` してレスポンスする。Rust 側には何もキャッシュしない。

レスポンスには `Cache-Control: max-age=3600` を付けているため、**WebView (wry → Chromium) の HTTP キャッシュに 1 時間バイナリが乗る**。これにより同じ URL の sprite を繰り返し表示してもファイル I/O が走らない。

### キャッシュ起因の落とし穴

URL が変わらないまま画像ファイルを上書きすると、WebView は最大 1 時間古いバイナリを表示し続ける。sprite 編集等で「保存したのに反映されない」と感じたらキャッシュを疑う。

将来エディタ機能で sprite を更新するようになったら、以下のいずれかで対処する想定:

1. URL に mtime / hash を query として付ける（`?v={mtime}`）→ URL が変われば自然に再フェッチされる
2. レスポンスを `Cache-Control: no-cache` に切り替える（desktop 内のループバックなので転送コストはほぼゼロ）
3. 編集アクション後に WebView のキャッシュを明示クリア

現状は sprite を読み取り表示するだけなので 1 時間キャッシュで問題なし。

---

## ファイルシステムレイアウト

`workspace/data/characters/{name}.yml` + `workspace/data/characters/{name}/sprite-groups/{group}.yml` の nested 構造。Character が集約ルート、SpriteGroup（と将来 SoundGroup）が子集約。

```
workspace/data/characters/
├── MooR_01.yml                              ← Character metadata (name, hp, thumbnail_path)
└── MooR_01/
    ├── sprite-groups/
    │   ├── walk.yml                         ← SpriteGroup metadata + sprites リスト
    │   └── walk/sprites/walk_001.png        ← 実バイナリ (use_asset_handler 経由で表示)
    └── animations/
        └── walk.yml                         ← Animation metadata (number, is_loop, frames[])
```

Character の YAML には sprite_groups / animations の情報を持たない（**Single Source of Truth**: 各子集約ファイルが正）。`Repository::get(name)` 時にディレクトリ走査で populate する。

Animation の `layers[]` は `sprite_group_number` + `sprite_index` で sprite を参照する（name / filename ではなく）。これにより SpriteGroup を rename しても、Sprite の filename を変更しても Animation は壊れない。

### ファイル所有関係（書き込み責務の境界）

子集約ディレクトリ (`sprite-groups/` / `animations/`) には `*.yml` (metadata) とサブディレクトリ（実バイナリ等）が同居しうるが、**書き込み責務は別**:

| パス | 書き込みオペレーション |
|---|---|
| `sprite-groups/*.yml` | `CharacterRepository::create` / `update` |
| `sprite-groups/{group}/sprites/*.png` 等 | sprite アップロード等の別オペレーション |
| `animations/*.yml` | `CharacterRepository::create` / `update` |
| `animations/{anim}/...` 配下（将来用） | 別オペレーション（layer 別キャッシュ等の想定） |

このため `Character.update()` は子集約ディレクトリを `remove_dir_all` で丸ごと消してはいけない（実バイナリが巻き添えになる）。`*.yml` のうち現在の集約名に含まれないものだけを削除し、サブディレクトリは触らない方針。リグレッションテスト: `filesystem_update_preserves_sprite_image_files` / `filesystem_update_preserves_files_under_animation_subdirs`。

---

## 将来の拡張可能性

- **キャッシュ層**: Repository の前段に `EntityCache<T>` (`RwLock<Option<Vec<T>>>`) を挟めば lazy + read-through キャッシュにできる。Repository インターフェースは同期のままで導入可能
- **オフロード**: ファイル I/O が重くなったら、`std::thread::spawn` で別 OS スレッドに逃がして結果を Signal に書き戻す。`async fn` / `tokio::task::spawn_blocking` には移行しない（editor は async/await を採用しない方針）

  ```rust
  // event handler 内
  let mut result = result_signal; // Signal<Option<T>> は Copy + Send
  std::thread::spawn(move || {
      let value = repo_clone.heavy_compute();
      result.set(Some(value)); // Signal は Send なので別スレッドから set できる
  });
  ```

- **個別 Subscriber**: 現在は「全体再フェッチ」一択だが、エンティティ単位の差分更新に進化させる余地あり
