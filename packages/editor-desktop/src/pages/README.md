# pages — URL 単位の thin controller

`Routes` enum (→ [`app/routes.rs`](../app/routes.rs)) の各 route と 1:1 で対応する component を置く。FSD のレイヤー的には `app` の直下、`widgets` の上。

## 役割と非役割

| やる | やらない |
|---|---|
| URL パラメータ (`name`, `group`, `anim`) を受け取る | URL 設計そのもの (`Routes` enum の編集は `app/routes.rs`) |
| URL から entity を解決する (`repo.get(&name())` など) | entity の永続化や CRUD ロジック (entities / features の責務) |
| 解決結果を widget に prop で渡す | UI 構造の組み立て (widgets の責務) |
| ロード中 / エラー / not-found のフォールバック表示 | layout (サイドバー / 左 rail) — `app/{section}_layout.rs` |
| `refresh` Signal を購読して再フェッチ | `bump()` を呼ぶ side (features の責務) |

**page は「URL → entity → widget」の細い controller** に徹する。新しい UI を組みたくなったら widget を作って page から render する。新しいユースケース (Save, Delete 等) は features 側に書く。

## 現存ページ

すべて [`characters.rs`](characters.rs) に同居。Character 集約に紐づくページがそろってここにあるのは、URL prefix `/characters` 配下で同じ Repository / refresh trigger を読むため。Character 以外のセクション (例: Settings ページ群) を増やすときは別ファイル (`pages/settings.rs` 等) に分ける。

| Page | Route | 渡す widget |
|---|---|---|
| `CharactersIndex` | `/characters` | （プレースホルダのみ） |
| `CharacterDetailPage` | `/characters/:name` | `CharacterDetail` |
| `SpriteGroupEditorPage` | `/characters/:name/sprite-groups/:group` | `SpriteGroupEditor` |
| `AnimationEditorPage` | `/characters/:name/animations/:anim` | `AnimationEditor` |

## 共通パターン: URL → entity の解決

データ取得の page は同じ骨格を踏む:

```rust
#[component]
pub fn CharacterDetailPage(name: ReadSignal<String>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let refresh = use_characters_refresh();
    let mut character = use_signal(|| None::<Character>);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        // bump() で再フェッチさせるために subscribe する
        let _ = refresh.subscribe();
        match repo.get(&name()) {
            Ok(found) => { character.set(found); error.set(None); }
            Err(e)    => error.set(Some(e.to_string())),
        }
    });

    rsx! {
        if let Some(message) = error() { /* alert */ }
        if let Some(c) = character()   { CharacterDetail { character: c } }
        else if error().is_none()      { /* spinner */ }
    }
}
```

ポイント:

- **URL パラメータは `ReadSignal<String>`** で受ける。Routable 由来。素の `String` で受けると `use_effect` がリアクティブに反応しない (CLAUDE.md の Dioxus RSX 規約)。
- **`refresh.subscribe()` を `use_effect` 先頭で呼ぶ** (→ ADR-0004)。これを忘れると features 側で `bump()` しても再フェッチされない。
- **エラーは Signal 化して inline alert で表示**。Toast に流すのは features の write 経路で、page の read 経路は inline。
- **not-found 表示は page 内で完結**: `c.animations.iter().find(|a| a.name == anim())` のように URL の child key (animation 名 / sprite_group 名) が当たるかを page 側で確認し、無ければイタリックで「'X' が 'Y' に見つかりません」。Animation / SpriteGroup を独立 entity に昇格していないため、親 Character を取って絞り込む形になる。

## Layout との責務分担

`CharactersLayout` (→ [`app/characters_layout.rs`](../app/characters_layout.rs)) は **`/characters/...` 配下で常駐するサイドバーの shell**。layout 自身も `repo.list()` を独自に呼ぶ。layout と page で同じ Character をそれぞれフェッチするのは冗長に見えるが、

- layout が必要とするのは「全 Character の概要 (sidebar 表示)」
- page が必要とするのは「単一 Character の詳細 (sprite_groups / animations 込み)」

で取得する形が違うため、共有せず両方が `refresh` を購読する形にしている (`repo.list()` は sprite_groups を載せない軽量版、`repo.get()` は載せる版という非対称)。

新セクション追加時の手順は [`app/README.md`](../app/README.md) の「新セクション追加」を参照。

## 関連 ADR / リファレンス

- ADR-0004: Refresh トリガー（`refresh.subscribe()` の根拠）
- `.claude/docs/data-flow.md`: Repository ⇄ Signal ⇄ Refresh の流れ
- `.claude/docs/ooui-fsd.md`: pages / widgets / features の役割分担
- [`app/README.md`](../app/README.md): ルーティング階層と layout 責務
