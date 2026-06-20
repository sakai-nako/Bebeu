# widgets/character/ui — Character / SpriteGroup / Animation 編集 UI の設計

このモジュールは Character の閲覧・編集画面を構成する widget 群を持つ。シンプルな一覧 / 詳細表示はコードを読めば足りるが、SpriteGroup 編集画面（`SpriteGroupEditor` + `SpriteCanvas` + `SpritePropertyPanel`）と Animation 編集画面（`AnimationEditor` + `AnimationCanvas` + `AnimationTimeline` + `AnimationPropertyPanel`）は座標系・state 共有・drag state machine が非自明なのでここで解説する。

## コンポーネント構成と状態の所有

```
SpriteGroupEditor                   ← state owner
├── SpriteEditorSidebar             ← Sprite 一覧（クリックで切替）
├── SpriteCanvas                    ← 画像 / HitBox / Pivot の可視化、drag 操作
└── SpritePropertyPanel             ← 数値編集、Box 一覧
```

すべての共有状態は `SpriteGroupEditor` で `use_signal` し、子に Signal で渡す。子は **Canvas でも Panel でも同じ Box を選択編集できる**（双方向）。

| Signal | 役割 |
|---|---|
| `Signal<SpriteGroup>` (draft) | 編集中のドラフト。Save まで disk に書かない |
| `ReadSignal<usize>` (selected_sprite_index) | サイドバーで選択中の Sprite |
| `Signal<Option<SelectedBox>>` (selected_box) | Body / Attack box の選択状態 |
| `Signal<Option<DragState>>` (dragging) | drag の進行状態（後述） |
| `Signal<Vec<SpriteReference>>` (references) | Reference 表示の設定（後述） |
| `Signal<CanvasVisibility>` (visibility) | Canvas マーカーの表示・非表示（後述） |

Refresh / 永続化は features 層の `SpriteGroupEditorActions`（Save / Cancel / Add Box）が担当する。widgets 層は disk に触らない。

## 座標系（3 つを使い分ける）

| 系 | 単位 | 出処 | 用途 |
|---|---|---|---|
| **image-pixel** | px (zoom なし) | `Sprite.pivot_point`, `HitBox.x/y` の永続値 | YAML 保存値、Box 操作の基準 |
| **canvas-pixel (CSS)** | px | canvas root 内のレイアウト座標 | pan の単位 |
| **viewport (client)** | px | `MouseEvent::client_coordinates()` | マウス delta の測定基準 |

`SpriteCanvas` は中央のラッパー div に以下の transform をかける（`transform-origin: 0 0`）:

```
transform: translate(pan_x, pan_y) scale(zoom) translate(-pivot_x, -pivot_y);
```

CSS は **右から評価される** ので、論理的な順序は「pivot を canvas 中央に寄せる → zoom で拡縮 → pan で平行移動」。delta 変換のルールは:

- 同一フレームでの delta なら viewport-pixel ↔ canvas-pixel は同値（pan の差分は同じスケールで効くため）
- canvas-pixel → image-pixel: `delta / zoom`

`delta_zoomed(diff_px, zoom)` がこの変換を担う。

## Drag state machine

`SpriteCanvas` が `Signal<Option<DragState>>` を持つ。`DragState` は drag 種別と origin を保持する:

```rust
pub struct DragState {
    pub kind: DragKind,
    pub start_mouse: [i32; 2],  // client coords (viewport)
}

pub enum DragKind {
    MoveSprite { start_pivot: [i32; 2] },                              // Pivot マーカーを掴んだ
    MoveBox    { target: SelectedBox, start: HitBox },
    ResizeBox  { handle: ResizeHandle, target: SelectedBox, start: HitBox },
    PanCanvas  { start_pan: [f64; 2] },
}
```

遷移:

```
                        mousedown(primary)
[None] ─────────────────────────────────► [Some(...)]
        Pivot marker → MoveSprite              │
        Box overlay  → MoveBox                 │  mousemove (kind / start_mouse は固定)
        resize handle (8 点) → ResizeBox       │  delta = client_xy - start_mouse
        canvas root + pan button → PanCanvas   │  apply per kind via apply_sprite_drag
                                                │
[Some(...)] ◄───────────────────────────────────┘
   │ mouseup / mouseleave
   ▼
[None]
```

設計の肝:

- **`mousemove` は `dragging` を変更しない**。kind と origin は drag 中ずっと固定。
- 結果として、`SpriteCanvas` のレンダーで `dragging()` をリアクティブ read しても、再レンダーは mousedown / mouseup でしか走らない。drag 中の毎フレーム再レンダーは発生しない。
- `start_pivot` / `start: HitBox` を kind 内に閉じ込めることで、delta 適用が「常に origin に対する加減算」になり累積誤差が出ない。

理由詳細:

- 座標系の選択 → ADR-0007（client_coordinates）
- Pivot 操作の起点 → ADR-0006（マーカー専用化）

## Drag 中の視覚フィードバック

`SpriteCanvas` のレンダー先頭で `dragging()` から派生値を作り、子に props で配る。

| Drag 状態 | 画像 | アクティブ Box | 他の Box | Pivot マーカー |
|---|---|---|---|---|
| なし | 通常 | 通常 | 通常 | primary 色 |
| MoveSprite | 通常 | — | 40% dim | warning 色 |
| MoveBox / ResizeBox | 70% dim | 通常 | 40% dim | 40% dim + primary |

`HitBoxOverlay` は `dimmed: bool` を受け取って opacity だけ切り替える単純な component。

## Box の index バッジ（zoom 不変表示）

各 HitBox の左上に `B0` / `A0` のラベルを置く。バッジは Box overlay の子なので親の `scale(zoom)` を継承してしまう。これを打ち消すため、transform をかけているラッパー div に CSS variable をセットする:

```rust
style: "... --zoom-inv: {1.0 / zoom_value};"
```

バッジ側で:

```html
transform: scale(var(--zoom-inv)); transform-origin: 0 0;
```

`scale(zoom) * scale(1/zoom) = scale(1)` で打ち消され、zoom が変わっても画面上のバッジサイズは一定。`transform-origin: 0 0` で Box の左上に張り付かせる。

## HitBox の永続化規約

`Sprite.body_boxes / attack_boxes` は `Option<Vec<HitBox>>`。空なら **`None` を保存する**（YAML に空配列を書かない）。`SpriteGroupEditorActions::on_save` が保存直前に `Vec::is_empty()` を検査して `None` に正規化する。読み込み側は `as_deref().unwrap_or(&[])` で素直に slice として扱う。

## 関連 ADR

- ADR-0006: Pivot manipulation via dedicated marker only
- ADR-0007: Drag tracking uses client coordinates
- ADR-0014: Frame override の 3-state encoding (`Option<Vec<HitBox>>`)

## AnimationEditor（Animation 編集画面）

Animation Editor は SpriteGroupEditor と同じ「Stay-on-screen Editor」パターン。`AnimationEditor` が state owner となり、`Signal<Animation>` (draft) を子コンポーネントへ渡す。

```
AnimationEditor                    ← state owner
├── AnimationCanvas                ← 選択 Frame の Layer 合成プレビュー（静的）
├── AnimationPropertyPanel         ← Frame props / Layer 一覧 / 選択 Layer 編集
└── AnimationTimeline              ← 下部の Frame strip（追加 / 削除 / 並べ替え / 複製）
```

| Signal | 役割 |
|---|---|
| `Signal<Animation>` (draft) | 編集中のドラフト。Save まで disk に書かない。Canvas / Panel 両方が読み書き |
| `Signal<Animation>` (baseline) | 最後に保存した状態。dirty 判定の基準（`draft != baseline`） |
| `UseHistory<Animation>` (history) | undo / redo（preferences の `animation_history_capacity` から取得）。Canvas (drag mousedown) / Panel (各 onchange) / Timeline / Actions の全部に渡す |
| `Signal<usize>` (selected_frame_index) | timeline で選択中の Frame |
| `Signal<Option<usize>>` (selected_layer_index) | プロパティパネルで選択中の Layer |
| `Signal<Option<SelectedBox>>` (selected_box) | Canvas / Panel が双方向同期する Override box の選択状態（→ Selection 同期節） |
| `Signal<Vec<SpriteReference>>` (references) | Reference 表示の設定（後述） |
| `Signal<CanvasVisibility>` (visibility) | Canvas マーカーの表示・非表示（後述） |

dirty 判定は `use_effect` で `NavigationGuard::set_blocked` に同期し、breadcrumb / 左 rail / Cancel 等あらゆるナビ起点で確認ダイアログが出るようにする。

### Frame.index / Layer.index は配列順序＝index で正規化する

任意の add / delete / move / duplicate のあとに `renumber_frames` / `renumber_layers` を呼んで `index = 0..N` を維持する。これにより `frames[selected_frame_index].index == selected_frame_index` が常に成り立ち、`for` でも `enumerate` でも添字混乱が起きない。

### AnimationCanvas のレンダリング

選択 Frame を中心に、static preview + interactive editor として描画する。Pivot Offset (Frame / Layer) と Body/Attack の Override box は drag/resize で編集できる (アニメーション再生は未実装)。ホイールでズーム、preferences で指定したマウスボタンでパン。

**描画レイヤー (DOM 順 = 後ろが手前)**

zoom/pan transform 内:

1. Back references
2. Layer images (`layer.index` 昇順) ＋ 各 image に重ねる元画像外枠 (`vis.image_frame`、LayerView 内で描画)
3. Inherit boxes (override が `None` の系統だけ、各 layer の sprite box を read-only で重ねる)
4. Override boxes (interactive、Body → Attack の順)
5. Front references

zoom/pan transform 外 (画面サイズ固定):

6. Origin marker (`pointer-events-none`、薄グレーの cross + dot、`frame_offset = 0` のリファレンス)
7. Frame Pivot Offset marker (cross の腕のみ interactive)
8. Sprite Pivot markers (per layer、Layer Pivot Offset の drag handle)
9. Frame info badge (左上)

各 Layer は `Character::find_sprite(layer.sprite_group_number, layer.sprite_index)` で sprite を解決。見つからない layer は赤いプレースホルダ。`transparency` は `style: opacity`。

`flip` (Frame.flip / Layer.flip) の意味論:
- **Layer.flip**: 該当 layer の sprite を `sprite.pivot_point` 中心に反転 (CSS では img の `transform-origin: sprite.pivot * zoom px`)。同じ layer の Inherit box (sprite が持つ body/attack box) も同じ pivot で反転して表示される。
- **Frame.flip**: frame 全体 (全 layer の sprite と Override box) を `frame_pivot` 中心に反転 (CSS では LayerView wrapper の `transform-origin: frame_offset * zoom px`)。各 layer の Inherit box にもこの反転が乗る。
- 両方指定すると「sprite-local → frame 座標 → frame-global」の順で合成。`resolve_inherit_box_in_frame` (animation_canvas.rs) と engine 側 `resolveInheritBoxInFrame` (flip_resolve.go) で同じ計算を行い、editor preview と engine 実機表示が一致する。

### AnimationCanvas の座標系まとめ

flip 適用前の **state 座標** (= YAML に書く値) を基準にすると:

| 対象 | 位置 (canvas 中央を 0,0 とする, flip 未適用) |
|---|---|
| Layer image 左上 | `-sprite.pivot + frame_offset + layer_offset` |
| **Override box** top-left | `frame_offset + box.top_left` (frame-pivot 相対 — layer に依存しない) |
| **Inherit box** top-left | `-sprite.pivot + frame_offset + layer_offset + box.top_left` (sprite 相対 — layer ごと) |
| Origin marker (画面) | `pan` |
| Frame Pivot marker (画面) | `pan + frame_offset * zoom` |
| Layer Pivot marker (画面) | `pan + (frame_offset + layer_offset) * zoom` |

Override box が **frame-pivot 相対**なのは「Frame で box を上書きする = layer 構成に依存せず Frame レベルで box を定義する」という意味。一方 Inherit box は各 layer の sprite で定義された box をそのまま見せるので **per-layer**。

Frame.flip / Layer.flip が `Some` のときの画面位置は、上表の値に対して:
- Override box: `box.top_left → flipped_around([0, 0], frame_flip)`、表示後に `+ frame_offset`
- Inherit box: `box.top_left → flipped_around(sprite.pivot, layer_flip) → -sprite.pivot + layer_offset で frame 座標化 → flipped_around([0, 0], frame_flip)`、表示後に `+ frame_offset`

drag/resize 中も画面上は flip 後の見た目で動かすが、`apply_frame_drag` 内で `invert_drag_delta` / `invert_resize_handle` で delta を逆 flip してから state に書き戻すため、state 自体は常に反転前の値で保持される (`flipped_around` が involution なので往復で一致)。

### Frame override の 3 状態

`Frame.body_box_overrides` / `attack_box_overrides` は `Option<Vec<HitBox>>` で 3 状態を表現する（→ ADR-0014）。

| 状態 | データ表現 | Canvas 描画 |
|---|---|---|
| Inherit | `None` | 各 layer の sprite から body/attack box を read-only で表示 |
| Override | `Some(non-empty)` | frame-pivot 相対の interactive box (drag/resize) |
| Disable | `Some(empty)` | 何も描かない (sprite の box も含めて完全に無効化) |

UI 上は `overrides.rs::BoxOverrideSection` のラジオボタン 3 択で切り替える。Override に切り替える際、既存値が `None` または空なら 16×16 のデフォルト box を 1 個自動追加する (`Some(empty)` のまま放置すると Disable と区別不能になるため)。

### AnimationCanvas の Drag state machine

`AnimationCanvas` がローカルに `Signal<Option<DragState>>` を持つ。SpriteCanvas と同じパターン (`client_coordinates`、kind / start は drag 中ずっと固定、累積 delta を origin に再適用) だが kind が広い:

```rust
pub enum DragKind {
    PanCanvas        { start_pan: [f64; 2] },
    MovePivotOffset  { start_offset: [i32; 2] },                              // Frame
    MoveLayerPivotOffset { layer_index: u32, start_offset: [i32; 2] },        // 各 layer
    MoveOverrideBox  { target: SelectedBox, start: HitBox },
    ResizeOverrideBox { handle: ResizeHandle, target: SelectedBox, start: HitBox },
}
```

`apply_frame_drag(frame, kind, dx, dy)` が drag 種別ごとに frame を更新する。`MoveLayerPivotOffset` は `frame.layers` の物理位置ではなく `layer.index` で対象 layer を探す（並べ替え耐性）。`pivot_point_offset` は `[0, 0]` のとき `None` に正規化して YAML の null と一致させる。

`history.record()` は drag 開始時 (mousedown) に呼ぶ。drag 中の毎フレームでは記録しない (= 1 drag = 1 undo step)。

### Pivot marker のクリック領域分離

Frame Pivot と Layer Pivot は `layer_offset = 0` のとき canvas 上で同位置に来るので、click target を**形状と pointer-events で分離する**。

| マーカー | 形状 / サイズ | クリック対象 | 用途 |
|---|---|---|---|
| Origin | cross + 中央 dot / 20×20 | なし (`pointer-events-none`) | `frame_offset = 0` のリファレンス |
| Frame Pivot | cross / 28×28 wrapper、腕 4×28 と 28×4 | **腕の visible 部分のみ** (wrapper は `pointer-events-none`、腕に `pointer-events-auto`) | drag で `frame.pivot_point_offset` を更新 |
| Layer Pivot | filled dot / 10×10 円 + 2px ring | dot 全体 | drag で対応 layer の `pivot_point_offset` を更新 |

ホバー feedback:

- Frame の腕は `group-hover:fill-warning` で**両腕同時にハイライト** (片方の腕にカーソルが乗ったら全体が点く)
- Layer dot は SVG 内に `opacity-0 group-hover:opacity-100` で warning stroke リングをもう 1 段重ねて、外周リングのオン/オフを切り替える (旧 `hover:ring-*` 相当)

これにより **drag 開始前** にどちらを掴もうとしているかが視認できる。

中央 4×4 px は Frame の腕と Layer dot が幾何的に重なるが、Layer dot が DOM 順で上にあり `pointer-events` の最上位ヒット判定で勝つ。結果として「**中央クリック → Layer drag、腕の他部分クリック → Frame drag**」が自然に成立する。Frame wrapper の最外要素を `pointer-events-none` にしているのは、透明な四隅をクリックした際に誤って Frame drag が起きないようにするため。

#### Marker は SVG で描く (4K / 高 DPI で半 px ズレを防ぐ)

Origin / Frame Pivot / Layer Pivot および SpriteCanvas の Pivot Marker はすべて SVG (`<svg>` + `<rect>` / `<circle>` + `fill-{color}` / `stroke-{color}` Tailwind utilities) で描いている。理由は次の通り:

- マーカー中央 "+" を「pivot 画素の中央」に乗せるため、box 位置に zoom/2 CSS-px の subpixel オフセットが入る (zoom = 1 のとき 0.5 CSS-px)
- 高 DPI ディスプレイ (例: 4K + 150% スケール) ではこの subpixel CSS-px が device pixel grid に合わず、DOM div だとブラウザがレンダーごとに違う方向に pixel-snap してしまい、frame / sprite を切り替えると 0.5 px ぶん上下に揺れて見える
- SVG の内部 rasterizer は subpixel 位置でも一貫した anti-alias 描画になるので、frame ごとの揺れが出ない

実装規約:

- SVG box の幅/高さは旧 div の bounding box と同じ (28×28 / 20×20 / 10×10)
- 旧 `border-2 border-base-100` 相当は `<circle stroke-width="2" class="stroke-base-100" />` に置き換える。`r` は visible outer 直径から逆算 (例: 10×10 dot なら r=4 + stroke-width=2 で外径 10、可視 fill 直径 6)
- 旧 `hover:ring-2 hover:ring-offset-1 hover:ring-warning` 相当は SVG 内にもう 1 段 `<circle stroke="warning" fill="transparent" />` を `opacity-0 group-hover:opacity-100` で出す。`overflow: visible` を SVG に付けて viewBox 外まで描けるようにする
- HTML の `title:` 属性は Dioxus で SVG 内ではコンフリクトするので `aria-label` を使う

### Drag 中の視覚フィードバック (dim ルール)

| Drag 状態 | Frame Pivot | Layer Pivot | Override box | Inherit box |
|---|---|---|---|---|
| なし | primary | secondary | 通常 | 60% 透過 |
| Frame Pivot drag | warning (active) | 40% dim | 40% dim | 30% 透過 |
| Layer Pivot drag (対応 layer) | 40% dim | warning (active) | 40% dim | 30% 透過 |
| Layer Pivot drag (他 layer) | 40% dim | 40% dim | 40% dim | 30% 透過 |
| Override box drag (active box) | 40% dim | 40% dim | 通常 | （描画なし） |
| Override box drag (他 box) | 40% dim | 40% dim | 40% dim | （描画なし） |

「主役が誰か」を視覚で示す規約。SpriteCanvas の dim ルールと同じ思想。

### Selection 同期 (`selected_box`)

`AnimationEditor` で `Signal<Option<SelectedBox>>` を `use_signal` し、Canvas / PropertyPanel の両方に渡す:

- Canvas の OverrideBoxOverlay を mousedown → `selected_box.set(Some(target))` → Panel の `BoxRow` 行が `bg-warning/10 ring-1` でハイライト
- Panel の `BoxRow` をクリック → `selected_box.set(Some(target.to_selected(box_index)))` → Canvas の対応 box に右下 8×8 リサイズハンドルが出る
- Canvas root を Primary クリック (どの marker にも当たらない) → `selected_box.set(None)` で deselect

SpriteCanvas + SpritePropertyPanel と同じ機構。`SelectedBox` enum (`Body(usize) | Attack(usize)`) を再利用している。

### 保存ライフサイクル

`AnimationEditorActions` (features 層) が Save / Cancel / Undo / Redo / Ctrl+S を担当。Save は **`update_animation()`** で `animations/{anim}.yml` だけを書き直す（`update()` で Character 全体を書き直すと他の SpriteGroup や Animation の mtime が動いてしまう）。

## Canvas マーカーの表示・非表示（CanvasVisibility）

SpriteCanvas / AnimationCanvas に重ねる各種マーカー（Pivot / Box / Reference / Origin / 元画像外枠）の表示を session 内で切り替える。`canvas_visibility.rs` に型と UI を置く。

- `CanvasVisibility`: 両 canvas のマーカー種別を **superset** で 1 構造体に持つ bool の集合。`Default` は全て `true`（元画像外枠もデフォルト表示）。session 内のみ保持し disk には書かない（references と同じ扱い）
- `CanvasVisibilityBar`: canvas 左上に floating するトグルバー。`fields: Vec<Field>` で「その canvas が出すトグル項目と順序」を渡す。`All` ボタンは渡された fields 限定で一括 on/off する
  - SpriteCanvas: `Pivot / Body / Attack / Reference / ImageFrame`
  - AnimationCanvas: `FramePivot / LayerPivot / Body / Attack / Reference / Origin / ImageFrame`
- Sprite には単一 Pivot のみ・Origin 無し、Animation には Frame/Layer Pivot と Origin がある、とマーカー構成が違うので superset 方式にした。各 canvas は自分が参照しないフィールド（例: Sprite 側の `frame_pivot` / `origin`）を無視するだけ

### 元画像の外枠（ImageFrame マーカー）

「元画像の dimensions（幅×高さ）の矩形」を image にぴったり重ねて描く補助線。画像の余白（透明部分）込みの境界が分かる。

- 実装は image と **同じ `left/top/width/height/transform`** を持つ `box-border border border-dashed` の div を 1 枚重ねるだけ（SpriteCanvas は img 直後、AnimationCanvas は LayerView 内で layer ごとに）。`box-border` なので枠線は寸法の内側に乗り、画像境界とちょうど一致する
- `dimensions = None`（PNG header 読み取り失敗等）のときは寸法が 0×0 になり実質非表示
- AnimationCanvas では Layer ごとに描くので、複数 layer なら各画像の外枠が出る。flip は image と同じ transform を共有するので追従する

## Reference 表示（SpriteGroupEditor / AnimationEditor 共通）

編集中の Sprite / Frame に他の Sprite を **pivot 揃え** で重ね描く表示用の機能。`sprite_reference.rs` に共通の型と UI を置く。

| 要素 | 役割 |
|---|---|
| `SpriteReference` | 参照対象 (`sprite_group_number`, `sprite_index`)、`placement` (Front / Back)、`opacity` を持つ単純な struct |
| `ReferenceLayer` | 親 container の (0, 0) を「編集中 Sprite の pivot 位置」とみなし、reference 画像の pivot がそこに重なるよう絶対配置で `<img>` を出すコンポーネント |
| `ReferenceSection` | プロパティパネル末尾に置く設定 UI。Sprite Group / Sprite Index / Placement / Opacity と削除ボタン |

特性:

- `references: Signal<Vec<SpriteReference>>` は **editor のセッション内のみ** 保持し、disk には書かない。draft とは独立に持つので Save / Cancel / Undo / Redo の影響を受けない
- pivot 揃え: `ReferenceLayer` 自身は `(-ref.pivot_x, -ref.pivot_y)` の位置に img を出すだけ。container の作り方で「pivot 位置 = container の (0, 0)」を表現するのは呼び出し側の責任
  - SpriteCanvas: parent の transform に `translate(-cur.pivot)` が含まれるので、container を `left: cur.pivot.x; top: cur.pivot.y` に置けば原点が pivot になる
  - AnimationCanvas: 中央 (50%, 50%) が pivot 位置なので container を `left: 50%; top: 50%` に置く

### CSS の落とし穴

`ReferenceLayer` を実装する際にぶつかった 2 つの罠を覚えておくこと。

1. **CSS painting order と SpriteCanvas の編集 img の z-index**
   - 編集 img は static (`block` だけ) で wrapper に直に置いてある。これは Tailwind preflight が img に `max-width: 100%` を入れているためで、親の wrapper を 0×0 にしてしまう absolute 化を避けたい
   - だが static 要素は CSS painting order (CSS 2.1 §9.9) で positioned 要素より前に描かれるので、何も対策しないと **すべての** positioned reference が編集 img の上に出てしまう
   - 解決: SpriteCanvas の Back placement reference container に `z-index: -1` を付け、negative stack level (step 2) で paint させる。Front placement は `z-index: auto` のままで positioned descendants (step 6) として img より後に paint される。Box overlay は Front の後の DOM に置くことで Front の上に来る
   - AnimationCanvas は LayerView 自身が positioned wrapper なので、reference と layer がすべて positioned 同士になり DOM 順序で paint される (z-index 不要)

2. **Tailwind preflight の `max-width: 100%`**
   - `ReferenceLayer` の img は absolute で、親 container は 0×0 (absolute で size 指定なし)。preflight の `max-width: 100%` が `max-width: 0` と評価されて img が完全に消えるので、`style` で `max-width: none` を明示的に上書きしている

### 4K + 非整数 DPR (例: 150% スケール) 対応: SpriteCanvas は explicit pixel sizing

非整数 DPR (= 4K + 150% スケール = DPR 1.5x など) で `zoom = 1` 編集すると、1 image-pixel = 1.5 device px のせいで browser の rendering pipeline 上の subpixel 位置で sprite が揺れる問題があった。

**SpriteCanvas (sprite_canvas.rs)** はこの問題に対処するため、wrapper に `transform: scale(zoom)` を使わない構成にしている:

- viewport wrapper: `left: round(50%, 1px); top: round(50%, 1px); width: 0; height: 0; transform: translate(pan); will-change: transform;` (zoom なし、pan のみ)
- 編集 image: `<img style="left: -pivot.x*zoom px; top: -pivot.y*zoom px; width: sw*zoom px; height: sh*zoom px;">` で zoom を CSS px に直接乗算
- box overlay: `left: (box.tl-pivot)*zoom px; top: (...)px; width: w*zoom px; height: h*zoom px;` (canvas_common::EditorBoxOverlay, sprite_canvas::BoxOverlayWrapper)
- pivot marker: SVG を child 座標 `(zoom/2, zoom/2)` に置いて pivot 画素中央狙い (transform: translate(-50%, -50%) で中央寄せ)
- reference: `ReferenceLayer` も `zoom` prop を受けて `width: ref.dimensions.0*zoom px` 等で explicit sizing

**なぜこれで効くか**: `transform: scale(zoom)` を使うと browser は image を native サイズで rasterize した後 compositor で scale するため、`image-rendering: pixelated` が compositor の GPU filter に効かず、4K + 150% で sprite ごとに subpixel 位置がブレる。`width: sw*zoom px` の explicit sizing なら paint 時に 1 step nearest-neighbor で rasterize され、image-pixel が integer zoom で uniform に保たれる。

**`Sprite.dimensions: Option<[u32; 2]>`**: explicit sizing には image の natural width/height が必要なので、Sprite struct に `#[serde(skip)]` フィールドとして持たせ、`FilesystemRepository::get` 等の loader が PNG header から読んで埋める (`shared::read_png_dimensions`)。新規 import 系 UI (create_character / import_sprites) も同 helper で埋める。YAML には書かない。

**画像 import / scale 系の補助**: `reimport_sprites_scaled::scale_sprite` は dimensions も scale 倍率に合わせて更新する (実 PNG 生成と数値の整合は呼び出し側責任)。

**AnimationCanvas (animation_canvas.rs)** も同じ explicit pixel sizing 構成に揃えてある:

- viewport wrapper: SpriteCanvas と同じ (left/top: round(50%, 1px), width/height: 0, transform: translate(pan))
- LayerView: img の left/top/width/height を `*zoom` の CSS px で直接指定。flip 用 `transform: scale(...)` の transform-origin も `sprite.pivot_point * zoom px` の CSS 座標に変換
- InheritBoxOverlay / OverrideBoxOverlay: position_style に `left/top/width/height` を `*zoom` で直接指定 (旧 50% + transform: translate のパターンを廃止)
- Frame Pivot / Origin / Layer Pivot marker (SVG): viewport child 座標で `(target_image_pixel * zoom + zoom/2)` に置き、`transform: translate(-50%, -50%)` で SVG box を中央寄せ
- ReferenceLayer: `zoom` prop を受けて explicit sizing

## 関連リファレンス

- `.claude/docs/data-flow.md`: Repository / Signal / Refresh トリガーの全体像
- `.claude/docs/dioxus-reactivity.md`: Signal / ReadSignal / use_effect の使い分け
