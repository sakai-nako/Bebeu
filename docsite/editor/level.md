# Level

Level は Beat 'em up の 1 ステージを表すデータ。base 画像（背景）と、その上で Player がどこを歩けるか / どこで敵が出るか / カメラがどこから映すかを 1 つの YAML にまとめる。

## ファイルレイアウト

Level は workspace dir 配下の `data/levels/` のマスタープールに置く。

```
<workspace_dir>/data/levels/
├── ct.yml                   ← Level 本体
└── ct/
    └── base.png             ← 背景画像（base フィールドが指す）
```

- YAML のファイル名がそのまま Level 名になる（YAML 内に `name` フィールドは書かない）。
- base 画像は editor の「base 画像取り込み」操作で同名ディレクトリにコピーされる。`base.png` 以外の名前にもできるが、その場合は YAML の `base` フィールドに正確なファイル名を書く。
- 同じ Level を複数の Project から参照できる（Project の `levels:` 配列に Level 名を書く）。

## YAML フィールド

| フィールド | 単位 / 既定値 | 役割 |
|---|---|---|
| `base` | 文字列 / `"base.png"` | `{level_name}/` 配下の背景画像ファイル名 |
| `areas` | リスト / 既定 1 件 | 移動可能領域（後述） |
| `camera_start_x` | 画像 X (px) / `0` | Level 開始時にカメラ視界の左端に来る base 画像の X 座標 |
| `camera_start_y` | 画像 Y (px) / `0` | Level 開始時にカメラ視界の上端に来る base 画像の Y 座標 |
| `player_spawn_x` | 画像 X (px) / `0` | Player の初期 spawn 位置（base 画像ピクセル X、Y は常に地面） |
| `player_spawn_z` | 画像 Y (px) / `0` | Player の初期 spawn 位置（base 画像ピクセル Y = 奥行き） |
| `player_respawn_y` | 高さ (px) / `0` | 死亡時に再 spawn を始める高さ（0 = 地面で即復活、正の値 = 上空から落下） |
| `opponent_triggers` | リスト / 空 | 敵出現スケジュール（後述） |
| `gravity_scale` | 倍率 / 省略 (= 1.0 相当) | Level ごとの重力倍率。実効 gravity = `Character.physics.gravity * gravity_scale` |

省略したフィールドは自動的に既定値で埋まる。空の `opponent_triggers` や省略可能な `gravity_scale` は YAML に書かなくてよい。

## 座標系：base 画像ピクセル = world 座標

Level の座標フィールドは **base 画像のピクセル位置そのもの**で書く。「画像のこのピクセルにキャラを置く」がそのまま world 座標になり、カメラを動かさなければ画面上の位置とも一致する。

- **X**（横）: base 画像の左 → 右（ピクセル X）
- **Z**（奥行き）: base 画像の上 → 下（ピクセル Y）。**画像の下ほど手前、上ほど奥**。Up 入力で奥（上）へ、Down 入力で手前（下）へ歩く
- **Y**（高さ）: jump で上方向へ。地面は Y=0

画面上の表示位置は `screen_y = world_z - camera_y - world_y` で決まる。手前（画像下＝Z 大）ほど画面下に、ジャンプ（Y 大）すると画面上に描画される。X は `screen_x = world_x - camera_x`。

> 以前の版にあった `ground_screen_y` / `z_scale` という投影パラメータは廃止された（座標系が base 画像ピクセルに一本化された）。経緯は ADR-0023 (image-pixel-world-screen-unification) を参照。

## `areas`（移動可能領域）

`areas` は **1 辺平行台形** のリスト。各 area は手前側 Z (`near_z`、画像下＝大きい値) と奥側 Z (`far_z`、画像上＝小さい値) を上下 2 辺として、左右の辺だけが斜めにできる台形。複数指定すると **OR 合成**（どれか 1 つに入っていれば移動可能）。

```yaml
areas:
  - near_z: 200.0     # 手前 = 画像の下のほう（大きい値）
    far_z: 80.0       # 奥 = 画像の上のほう（小さい値）
    near_min_x: 0.0
    near_max_x: 640.0
    far_min_x: 0.0
    far_max_x: 640.0
```

形のイメージ（俯瞰。画像の上が奥、下が手前）:

```
              z = far_z   (奥 / 画像上、小さい値)
            +-----------+
            |           |       far_min_x .. far_max_x
            |  AREA     |
           /             \
          /               \
         +-----------------+    near_min_x .. near_max_x
              z = near_z   (手前 / 画像下、大きい値)
```

- `near_min_x == far_min_x && near_max_x == far_max_x` のときは矩形になる。
- 区切られた島や合流通路は area を 2 つ以上並べて表現する。
- `near_z < far_z`（手前が奥より小さい）や `near_min_x > near_max_x` のような不正値は engine 起動時にエラーになる。

設計の経緯と代替案は ADR-0022 (level-area-one-side-parallel-trapezoid-or) を参照。

## `opponent_triggers`（敵出現スケジュール）

Player の world X が `trigger_x` 以上になった瞬間に 1 回だけ発火し、`(spawn_x, spawn_y, spawn_z)` に `character_name` の Character を生成する。

```yaml
opponent_triggers:
  - character_name: MooR_02
    trigger_x: 200.0
    spawn_x: 480.0
    spawn_y: 0.0
    spawn_z: 180.0
  - character_name: MooR_02
    trigger_x: 480.0
    spawn_x: 700.0
    spawn_y: 0.0
    spawn_z: 160.0
```

- 同じ trigger が再発火することはない（1-shot）。
- `character_name` は `<workspace_dir>/data/characters/` の Character 名そのまま。Character を rename / delete しても Level YAML 側は自動更新されないので、エディタ上の警告に従って手動修正する。

## 典型シナリオ

### A. 矩形だけのシンプル Level（既定 Area で十分）

```yaml
base: base.png
# areas / camera_start_* / player_spawn_* / player_respawn_y は省略可
```

### B. 奥行きで広がる台形 Level

```yaml
base: street.png
areas:
  - near_z: 200     # 手前（画像下）。広い
    far_z: 140      # 奥（画像上）。狭い
    near_min_x: 0
    near_max_x: 960
    far_min_x: 120
    far_max_x: 840
player_spawn_x: 80
player_spawn_z: 170
```

### C. 進行に応じて敵を順に出す

```yaml
base: base.png
opponent_triggers:
  - character_name: MooR_02
    trigger_x: 100
    spawn_x: 400
    spawn_y: 0
    spawn_z: 180
  - character_name: MooR_02
    trigger_x: 400
    spawn_x: 700
    spawn_y: 0
    spawn_z: 200
  - character_name: Boss_01
    trigger_x: 800
    spawn_x: 1000
    spawn_y: 0
    spawn_z: 190
```

## editor 上での編集

詳細な編集はエディタの Level 詳細ページ（`/levels/{name}`）で行う:

- 新規作成モーダルから名前と base 画像を指定して Level を作る
- Canvas 上で Area / Player spawn / OpponentTrigger を視覚的に操作する
- インスペクタで `camera_start_*` / `player_respawn_y` などの数値フィールドを編集する
- Save するまでは disk に書き込まれない（未保存編集を保護）

editor が書き出した YAML をテキストエディタで再編集することも可能。フィールド順や空行は保存時に正規化される。
