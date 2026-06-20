# widgets/level/ui — Level 編集 UI の設計

Level (1 ステージ) の視覚編集を担う widget 群。base 画像の上に **Area (移動可能範囲)**・**OpponentTrigger (敵出現)**・**Camera 開始位置**・**Player Spawn** を重ね、画面上でドラッグ編集する。zoom / pan を持つ。`widgets/character/ui` の SpriteCanvas / AnimationCanvas と同系の canvas widget で、座標系と drag state machine の考え方を共有する (差分は「sprite-pixel 平面」ではなく「base 画像 = world (X, Z) 平面」を編集する点)。

## ファイル構成と状態の所有

facade は `level/ui.rs` (`mod` + `pub use`)。

| ファイル | コンポーネント | 責務 |
|---|---|---|
| `level_detail.rs` | `LevelDetail` | Pattern D の状態オーナー。`draft: Signal<Level>` と `history: UseHistory<Level>` を生成し、Canvas / Inspector / `LevelEditorActions` に配る (thin) |
| `level_canvas.rs` | `LevelCanvas` | base 画像 + Area / Trigger / Camera / Player の描画とドラッグ編集。zoom / pan / drag state machine の本体 |
| `level_inspector.rs` | `LevelInspector` | 右サイドバー。Camera / Player / Physics / OpponentTriggers の各 inline 編集セクションを並べる (実体は `features/level/ui`) |
| `levels_sidebar.rs` | `LevelsSidebar` | 左 rail の Level 一覧 |

編集状態 (`draft` / `history` / dirty 判定) は `LevelDetail` が一元所有し、Canvas もフォーム (`features/level/ui`) も同じ `draft` Signal を共有する。Canvas のドラッグ結果と Inspector の数値入力が双方向に反映されるのはこのため。保存ライフサイクルは [`features/level/ui/README.md`](../../../features/level/ui/README.md) を参照。

## 座標系 (ADR-0026)

base 画像ピクセルが world (X, Z) と一本化されている (ADR-0026) ので、編集対象の永続値はすべて画像ピクセル単位。3 層を区別する:

| 層 | 単位 | 用途 |
|---|---|---|
| **world (X, Z)** | i32 (画像ピクセル) | Area / Camera / Player / Trigger の永続値。X=横、Z=奥行き (画像 Y) |
| **screen (画像相対)** | f64 | world と同値。CSS `transform: scale(zoom)` 適用前の描画座標 |
| **CSS-pixel** | f64 | マウスイベントの実ピクセル。drag delta と pan の計測に使う |

**drag delta の変換**: マウス移動量 (CSS-pixel) を `delta_css / zoom = delta_world` で world に直してから適用する。累積誤差を避けるため、mousemove では毎回「ドラッグ開始時のスナップショット + 累積 world delta」で再計算する (差分加算しない)。例外は `PanCanvas`: pan は CSS `transform: translate` に効くので、canvas-pixel delta を **zoom 補正なしで**そのまま加算する。

## Zoom と Pan

- **zoom 段階**: `LEVEL_ZOOM_LEVELS = [0.1, 0.125, 0.25, 0.333, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0]` の 16 段階。SpriteCanvas が subpixel 揃えのため整数 + 0.5 単位なのと違い、Level の base は pixel art とは限らず大きい写実的画像 (1920×1080 等) も載るので、縮小側と 1.0 周辺を細かくしている。
- **ホイール zoom**: `next_level_wheel_zoom(current, delta_y, invert)` が現在値に最も近い段 (`nearest_level_zoom_index`) の隣を返す。invert は `ViewControlBindings` (preference) から受ける。
- **canvas 中央固定**: zoom 変更時に `pan' = pan * (next_zoom / current_zoom)` で view 中心の world 座標が動かないよう pan を補正する。
- **コンテナ**: `transform: translate(pan_x, pan_y) scale(zoom)`。pan はセッション内の UI 状態で、`draft` にも history にも記録しない。

## Drag state machine

`DragState { kind: DragKind, start_mouse: [i32; 2] }` を Signal で持ち、`None` = アイドル。`DragKind` は 6 種で、それぞれ mousedown 時点のスナップショットを保持する:

| `DragKind` | スナップショット | 編集対象 |
|---|---|---|
| `AreaHandle { handle, start_area }` | Area 全体 | 台形の 4 頂点 (`AreaHandle::{NearLeft, NearRight, FarLeft, FarRight}`) |
| `CameraStartHandle { start_x, start_y }` | camera (X, Y) | Camera 開始位置 |
| `PlayerSpawnHandle { start_x, start_z }` | player (X, Z) | Player Spawn (Y は常に 0) |
| `TriggerLine { index, start_x }` | trigger_x | OpponentTrigger の発火閾値 (Player X) |
| `TriggerSpawnPoint { index, start_x, start_z }` | spawn (X, Z) | Trigger の敵出現位置 (Y は数値入力のみ) |
| `PanCanvas { start_pan }` | pan | canvas 視点移動 (未保存) |

遷移:

```
None ──mousedown(対象ハンドル)──▶ Some(DragState{ kind, start_mouse })
  │                                    │
  │                              mousemove: delta_world = (mouse - start_mouse)/zoom
  │                                        対象を「snapshot + delta」で再計算して draft を仮更新
  ▼                                    ▼
None ◀──────mouseup: history.record() してから draft 確定──────┘
```

- Area の頂点適用は `apply_area_drag(start_area, handle, dx_world, dz_world)`。near 系 2 頂点は `near_z` を、far 系 2 頂点は `far_z` を共有するので、台形の上下辺は常にスクリーン水平に保たれる。
- `base_dimensions` があれば `clamp_area_to_image` / `clamp_image_x` / `clamp_image_y` で画像の `[0, width] × [0, height]` 内に収める。

## Area: 1 辺平行台形の描画 (ADR-0025)

Area は `near_z` / `far_z` (奥行きの手前・奥) と `near_min_x / near_max_x / far_min_x / far_max_x` (各辺の左右端) を持つ台形。`near_min_x == far_min_x && near_max_x == far_max_x` のとき矩形に縮退する。SVG polygon の 4 頂点座標を生成して描画する。型の詳細・OR 合成のセマンティクスは [`entities/level/README.md`](../../../entities/level/README.md) を参照。

## OpponentTrigger の可視化

- **trigger_x**: 画像高さ全体を貫く破線縦線 (`stroke-dasharray`)。Player の world X がこの線を越えると engine が敵を spawn する。ドラッグ用の当たり帯は `width: 10/zoom px` で cursor `ew-resize`。
- **spawn 位置**: 敵が出る (X, Z) を小さなマーカーで表示。`transform: scale(1/zoom)` で zoom に依らず一定サイズに保つ (SpriteCanvas の index バッジと同じテクニック)。

## Project 解像度の参照表示

Camera 視界の矩形と trigger 縦線の高さは「どの Project (= 解像度) で見るか」で変わる。`ProjectRepository` からこの Level を参照する Project を集めて `referencing_projects: Vec<(name, width, height)>` を作り、ドロップダウンで選んだ解像度で camera 矩形を描く。Level 自身は解像度を持たない (Project が持つ) ので、編集中のプレビュー用にのみ使う。

## base 画像の explicit pixel sizing

`base_dimensions` (PNG ヘッダから読んで `#[serde(skip)]` で注入) を使い、`<img>` に `width: {w}px; height: {h}px; max-width: none;` を直接指定する。Tailwind preflight の `max-width: 100%` を打ち消し、zoom (`scale`) だけがサイズを決めるようにするため (SpriteCanvas / AnimationCanvas と同じ理由。memory: marker は SVG、画像は `<img>`)。

## History 記録規約

ドラッグ系はすべて **mouseup の確定時に `history.record()` してから `draft` を書く**。Area / Camera / Player / Trigger の各ドラッグが対象。Area の追加・削除ボタンも即 record する。`PanCanvas` (視点移動) は UI 状態なので記録しない。Undo/Redo の発火と Save は `LevelEditorActions` (features 層) が担う。

## 関連

- ADR-0025: Level Area = 1 辺平行台形のリスト + OR 合成
- ADR-0026: base 画像ピクセル = world (X, Z) = screen 座標の一本化
- `entities/level/README.md`: Area / OpponentTrigger の型と永続化レイアウト
- `features/level/ui/README.md`: CRUD と inline 編集フォーム、Pattern D の保存ライフサイクル
- `widgets/character/ui/README.md`: 同系の canvas (座標系 / drag state machine の原型)
