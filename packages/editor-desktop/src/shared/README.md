# shared — レイヤーをまたぐ汎用ヘルパー

FSD の最下層。`entities` / `features` / `widgets` / `pages` / `app` のどこからでも参照できる。逆に **shared から上の層を参照しない**（コンパイラが強制する）。

## 何を置くか / 置かないか

| 性質 | 置く | 置かない |
|---|---|---|
| ドメイン依存 | 複数 entity が共有する domain primitive（例: `HitBox` は SpriteGroup と Frame の両方で使う） | 単一 entity 専用の型（→ `entities/{entity}/`） |
| ユーザー操作 | キー解析や drag delta 計算など input の **技術的ヘルパー** | 「Save する」「Delete する」といったユースケース（→ `features/`） |
| UI | フックや CSS クラス計算など特定 widget に縛られない汎用 hook | 特定の UI 構造を持つコンポーネント（→ `widgets/`） |
| 永続化 | URL 規約や config ファイルのロード等のブートストラップ | ドメインデータの Repository（→ `entities/{entity}/api.rs`） |

迷ったら「**他レイヤーに移すと依存が増えるか？**」で判断する。`HitBox` を `entities/character` に置くと `entities/character` 以外（将来の `entities/animation_frame` 等）から参照できない。`history` を `widgets` に置くと `features` から使えない。

`shared` は **slice を持たない**（segment 直）。facade ファイル `shared.rs` が `mod` + `pub use` で各モジュールの公開 API を定義する。`shared::history::UseHistory` のように internal path を経由しない。

## 現存モジュール

| モジュール | 種別 | 公開 API | 用途 |
|---|---|---|---|
| `collision` | domain primitive | `HitBox`, `FlipMode`, `ResizeHandle` | 当たり判定矩形。`new` で `top_left ≤ bottom_right` に正規化、`translated` / `resized(handle, dx, dy)` / `flipped_around` で immutable 変換。`ResizeHandle` は canvas 上の 8 点リサイズハンドル (4 隅 + 4 辺中点) |
| `config` | bootstrap | `Config` | `bebeu-editor.yml` のロード（→ ADR-0012）。debug は CWD、release は exe 隣を見る |
| `history` | UI hook | `History`, `UseHistory`, `use_history` | Undo/Redo（→ ADR-0010、`.claude/docs/undo-redo.md`） |
| `image_cache_buster` | UI hook | `ImageCacheBuster`, `use_image_cache_buster_provider`, `use_image_cache_buster`, `versioned_asset_url` | 画像書き換え後に `<img src>` を `?v={N}` で再フェッチさせるカウンタ（→ ADR-0015）。provider 不在時は no-op |
| `keybinding` | input parser | `KeyBinding`, `KeyModifiers`, `KeyParseError` | キー文字列 ⇄ 構造体の往復、`from_keyboard_event` で Dioxus イベントから構築 |
| `view_controls` | input bindings | `PanButton`, `ViewControlBindings` | パン用ボタン + ホイール zoom。Zoom は固定倍率列 (0.25 / 0.5 / 整数) を `next_wheel_zoom` で階段する方式 — sprite が device pixel に対して subpixel 位置に着地して frame 切替で半 px ずれて見えるのを防ぐ。Preferences の 1 フィールドとして持つ |
| `webview_assets` | URL 規約 | `WORKSPACE_ASSET_SCHEME`, `WORKSPACE_ASSET_URL_PREFIX`, `workspace_asset_url` | webview から workspace 配下の画像を `<img src>` で読むための URL 規約（→ ADR-0005） |
| `toast` | UI hook + component | `ToastKind`, `UseToast`, `use_toast`, `use_toast_provider`, `ToastHost` | 画面右下に出すアクション結果通知。app_root で provider + ToastHost を 1 度 mount し、子は `use_toast()` で `success/error/warning/info` を呼ぶだけ。フォーム内バリデーション系のインライン `alert` は引き続き使う |
| `wav_header` | file parser | `WavInfo`, `read_wav_info`, `parse_wav_info` | WAV ヘッダから sample_rate / channels / bits / duration を読む軽量パーサ。`hound` 依存を避けるため自前実装。SoundGroupEditor で wav メタデータを表示する用途 |

## ドメイン primitive と entity の境界

`HitBox` を `shared/collision.rs` に置いている理由:

- `Sprite.body_boxes / attack_boxes`（SpriteGroup 集約配下）と、将来の `Frame.body_box_overrides`（Animation 集約配下）の両方で使う
- entity slice 同士は同レイヤーで参照禁止（FSD の encapsulation 原則）
- ドメインに依存しない計算（`translated` / `flipped_around` 等）に閉じている

逆に「Sprite を表す struct」「Animation を表す struct」は entity に縛られた集約構造なので `entities/character/model.rs` に置く。

## 関連 ADR / リファレンス

- ADR-0001: Adopt Feature-Sliced Design（レイヤー方針の根拠）
- ADR-0005: WebView asset handler with 1h cache（`webview_assets` の経緯）
- ADR-0010: Undo/Redo を editor session-scope の snapshot 履歴で実装する（`history` の経緯）
- ADR-0012: Two-tier configuration files（`config` の経緯）
- ADR-0015: Image cache busting via URL query（`image_cache_buster` の経緯）
- `.claude/docs/fsd.md`: FSD のレイヤー / slice / segment 全体像
- `.claude/docs/undo-redo.md`: `History` / `UseHistory` の使い方と呼び出し規約
