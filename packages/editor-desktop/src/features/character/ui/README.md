# features/character/ui — Character / SpriteGroup / Animation の CRUD UI

ユーザー操作の単位ごとに 1 ファイルを置く方針。各ファイルは「ボタン + ボタンが開く UI（モーダル / インライン入力 / 確認ダイアログ）」を 1 セットでカプセル化する。

## ファイル分割の方針

「ユーザーの 1 アクション = 1 ファイル」。UI 形態（modal / inline / confirm）は中身の詳細であって、ファイル粒度の基準ではない。

```
Character (集約ルート):
  create_character.rs        CreateCharacterButton
  rename_character.rs        RenameCharacterButton
  delete_character.rs        DeleteCharacterButton
  edit_hp_inline.rs          EditHpInline
  change_thumbnail.rs        ChangeThumbnailButton

SpriteGroup (子集約):
  create_sprite_group.rs     CreateSpriteGroupButton
  rename_sprite_group.rs     RenameSpriteGroupButton
  delete_sprite_group.rs     DeleteSpriteGroupButton
  edit_sprite_group_number_inline.rs  EditSpriteGroupNumberInline

Animation (子集約):
  create_animation.rs        CreateAnimationButton
  rename_animation.rs        RenameAnimationButton
  delete_animation.rs        DeleteAnimationButton
  edit_animation_role_inline.rs  EditAnimationRoleInline  ← CharacterDetail で Role / Variant を inline 編集

Sprite:
  import_sprites.rs              ImportSpritesButton          ← フォルダ選択で複数画像を新規 import
  reimport_sprites_scaled.rs     ReimportSpritesScaledButton  ← 倍率指定で同名画像群を再 import

SpriteGroup 編集ビュー専用:
  edit_sprite_group.rs       SpriteGroupEditorActions  ← Save / Cancel / Add Box

Animation 編集ビュー専用:
  edit_animation.rs          AnimationEditorActions    ← Save / Cancel / Undo / Redo
```

ファイル facade は `features/character/ui.rs` に集約され、上位 layer は `crate::features::character::CreateCharacterButton` のように slice facade 経由で import する。

## 3 つの UI パターン

### Pattern A: Modal（新規作成 / 改名）

ボタン → `dialog.modal-open` → form → submit → `repo.X()` → `refresh.bump()` + `onclose()`。

```rust
#[component]
pub fn CreateXButton() -> Element {
    let mut show_modal = use_signal(|| false);
    rsx! {
        button { onclick: move |_| show_modal.set(true), "+ New" }
        if show_modal() {
            CreateXModal { onclose: move |()| show_modal.set(false) }
        }
    }
}
```

モーダル本体は別 component（`CreateXModal`）に分離。`onclose: EventHandler<()>` でクローズを受け取り、外側のトリガー Signal を直接持たない（state 漏れを防ぐ）。

### Pattern B: Inline 編集（HP, group number 等の単項目）

表示モード ↔ 編集モードを `editing: Signal<bool>` で切り替え。draft / error も同様にローカル Signal:

```rust
let mut editing = use_signal(|| false);
let mut draft = use_signal(|| original.field.to_string());
let mut error = use_signal(|| None::<String>);
```

保存先の Repository API は**編集対象フィールドがどの yml に属すか**で決まる:

- HP / Depth / group number など **Character 自身のフィールド** → `update_metadata`（`update` ではない。理由 → `entities/character/README.md` の "Repository write API の責務範囲" 表）
- Animation の Role / Variant など **単一 Animation yml のフィールド** → `update_animation`（`edit_animation_role_inline.rs`）。他の animations/*.yml や character.yml の mtime を動かさない

`edit_animation_role_inline.rs` は表示 (色付き Role badge) を widget 側 (`AnimationRow`) に残し、feature は編集フォーム + 永続化だけを担う。`editing` Signal を親と共有し、editing 中だけ mount される（= draft の seed が常に最新）。Role の正規化規約 (single → variant=0 / Custom 以外 → export_number=None) は property panel の `AnimationRoleSection` と揃える。

### Pattern C: Confirm（削除）

削除は確認モーダルで二段階に。`features/character/ui/delete_character.rs` のように、ボタン → `DeleteConfirmModal` → `repo.delete()` → `refresh.bump()` + nav。

削除直後はそのリソースの詳細ページが無効になるので、`use_navigator().replace(parent_url)` で親へ戻す:

```rust
match repo.delete(&target) {
    Ok(()) => {
        refresh.bump();
        onclose.call(());
        nav.replace("/characters");
    }
    Err(e) => error.set(Some(e.to_string())),
}
```

### Pattern D: Stay-on-screen Editor（SpriteGroup / Animation 編集）

専用ページ全体を編集サーフェスとして使う場合のパターン。`features/character/ui/edit_sprite_group.rs` の `SpriteGroupEditorActions` と `features/character/ui/edit_animation.rs` の `AnimationEditorActions` がこれ。Pattern A〜C と違って **保存後も画面に留まる**ので、dirty 追跡と離脱確認が必要になる。AnimationEditor は **保存に `update_animation()` を使う**点だけが SpriteGroupEditor と異なる（`update()` だと他の SpriteGroup / Animation の mtime が動いてしまうため）。

ライフサイクル:

1. **`baseline` Signal**: 「最後に保存した状態」のローカルコピー。`use_signal(|| original_group.clone())` で初期化
2. **dirty 判定**: `let is_dirty = draft() != *baseline.read();` を rsx の直前に評価
3. **未保存バッジ**: dirty のとき Cancel と Save の間に `badge-warning` で「● 未保存」を表示
4. **保存処理** (`on_save`):
   - draft を正規化 (空 Vec → None) → `repo.update(&updated)` → 成功時に **`draft.set(current_draft)` + `baseline.set(current_draft)` + `refresh.bump()` + `error.set(None)`**
   - `nav.replace(detail_url)` は呼ばない（編集画面に留まる）
5. **Ctrl+S** は `use_keyboard_action(Action::Save, move || on_save.call(()))` で発火。`Save` は SpriteGroup / Animation / SoundGroup 共通の単一 Action で、mount された Editor の hook だけが register される。`on_save` を `use_callback` 化することで onclick / shortcut の両方から同じロジックを呼べる
6. **NavigationGuard 同期** (→ ADR-0008):
   - `use_effect(move || guard.set_blocked(draft() != *baseline.read()))` で dirty を guard に流す
   - `use_drop(move || guard.set_blocked(false))` で unmount 時に必ず解除
7. **Cancel ボタン** は `guard.try_navigate(&nav, detail_url.clone())` を呼ぶだけ。dirty なら RootShell のグローバル confirm が出る、そうでなければ即遷移する。ローカル confirm dialog は持たない (Pattern C と違う点)

なぜ `baseline` Signal を別に持つか: 親 (`SpriteGroupEditorPage`) は `refresh.bump()` 後に再フェッチして `original_group` prop を更新するが、その間にラグがあるため、保存直後の 1 フレームで `draft != original_group` と判定されてバッジが点滅する。`baseline` を保存と同期に更新することでブレない。

なぜ保存時に `draft` も正規化済み値で上書きするか: 空 Vec ↔ None の差で `draft != baseline` が成立してしまうため、両方を同じ正規化済み値に揃える。

#### `SpriteDiskOps` の commit / rollback

SpriteGroupEditor では yml 保存と並行して画像ファイルの disk 操作（import / 上書き / 削除）が走るので、Save と Cancel/unmount の両経路で disk を yml と整合させる必要がある。これを `SpriteDiskOps` で集約管理している:

- `pending_imports`: 新規コピーされた画像 basename。Save 後はクリア（commit）／Cancel で `delete_sprite_image` で消す（rollback）
- `pending_deletions`: 削除予定の画像 basename。Save 時に `delete_sprite_image` で実削除／Cancel では何もしない
- `pending_overwrites`: 同名上書き import で `{basename}.bak` のバックアップが取られた画像。Save で `discard_sprite_image_backup`（.bak 削除）／Cancel で `restore_sprite_image_backup`（.bak から復元）

`pending_overwrites` は倍率再インポート（`ReimportSpritesScaledButton`）のように既存 basename を上書きする操作で発生する。Save 経路は `edit_sprite_group.rs` の `on_save`、rollback 経路は `widgets/character/ui/sprite_group_editor.rs` の `use_drop` に集約されている。

## 共通の hook 取得

すべての feature で:

```rust
let repo = use_context::<Arc<dyn CharacterRepository>>();
let mut refresh = use_characters_refresh();
let nav = use_navigator();   // 削除など nav が必要な時のみ
```

repo は context から、refresh は専用 hook、nav は dioxus_router の標準 hook。features 層は **repo を直接生成しない**（DI されたものを使う）。

## エラーハンドリング

`Signal<Option<String>>` を `error` として持ち、Repository 呼び出しの `Err(e)` を `error.set(Some(e.to_string()))` で表示する。daisyUI の `alert alert-error` を使う:

```rust
if let Some(message) = error() {
    div { role: "alert", class: "alert alert-error",
        span { "{message}" }
    }
}
```

## 数値入力は String で持つ

`u32` に直接 `value:` バインドすると、ユーザーが「400」→「00」と編集する途中で 0 に丸まり、表示が壊れる。**入力 Signal は `String` で持ち、submit 時に `.parse::<u32>()` する**。

```rust
let mut sprite_group_number_input = use_signal(|| DEFAULT_SPRITE_GROUP_NUMBER.to_string());
// ...
let Ok(group_number) = sprite_group_number_input().trim().parse::<u32>() else {
    error.set(Some("0 以上の整数で入力してください".into()));
    return;
};
```

## 失敗時のロールバック（マルチステップ）

`create_character` のように 2 段階の作業（画像 import → Character create）がある場合、後段が失敗したら前段をロールバックする:

```rust
let basename = repo.import_sprite_image(...)?;        // 1. 画像コピー
match repo.create(&new_char) {                        // 2. Character 書き込み
    Ok(()) => refresh.bump(),
    Err(e) => {
        let _ = repo.delete_sprite_image(...);        // ← 1 を取り消す
        error.set(Some(e.to_string()));
    }
}
```

`delete_sprite_image` がロールバック専用に存在している（→ `entities/character/README.md`）。

## 重複検出は事前 + 事後の両方で

たとえば `create_character` は:

1. **事前**: `repo.get(&name)` で重複を検出してフォーム上のエラーを出す（早期に UX 良いフィードバック）
2. **事後**: それでも `repo.create` が `'X' already exists` を返したらエラー表示（race condition / 別プロセス対策）

事前チェックだけだと TOCTOU で抜ける。Repository 側にも重複拒否を実装してある。

## 関連リファレンス

- `entities/character/README.md`: Repository write API の責務範囲
- `entities/navigation_guard/README.md`: Pattern D で使う離脱確認 guard の API と典型フロー
- `.claude/docs/data-flow.md`: Refresh トリガーの仕組み / NavigationGuard / KeyboardActionDispatcher
- ADR-0004: なぜ Refresh が wrapping counter なのか
- ADR-0008: NavigationGuard 設計判断 (Pattern D の Cancel が global guard 経由になる理由)
- ADR-0009: キーボードショートカットのディスパッチ設計 (Ctrl+S が `use_keyboard_action` で発火する理由)
