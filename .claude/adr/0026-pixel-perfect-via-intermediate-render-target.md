# ADR-0026: pixel-perfect 拡大を中間 render texture で分離する (window は yml 駆動)

## Status

Accepted

## Context

engine の window pixel サイズは長らく `viewport (= project.resolution = 384×216) × 整数倍 (=3) = 1152×648` で決め打ちされていた。これは `nearest filter で source pixel : screen pixel = 1 : N (整数)` を保証して pixel art の rippling を防ぐためのポリシーだった ([entrypoint.rs](../../packages/engine/src/app/entrypoint.rs) の `WINDOW_INTEGER_SCALE` コメント参照)。

一方で `packages/engine/bebeu-engine.yml` には `window: { width: 1280, height: 720 }` が書かれていたが、loader が未実装 (ADR-0016 の placeholder 状態) で **engine から 1 行も読まれていなかった** (`shared::config::RuntimePaths::resolve()` は env と manifest fallback だけ参照)。結果として:

1. yml の設定値が dead config で、ユーザー期待と乖離していた
2. 整数倍ポリシーで window サイズが「viewport × N の離散値」に縛られ、フル HD 等を出せなかった
3. コメントは「nearest filter 前提」と書いてあるが実態は `ImagePlugin` 未設定 = Bevy デフォルト = **linear** で補間されていた (= 3 倍ピッタリで blur が目立たなかっただけ)

修正方針として:

- **(a) yml.window を直接 window サイズに採用**: 整数倍ポリシーを廃止。1280×720 (× 3.33) では nearest だと rippling、linear だとボヤけ
- **(b) 整数倍ポリシーを保ち倍率を上げる**: yml を無視、4× (1536×864) / 5× (1920×1080) 等。yml が dead のまま
- **(c) 中間 render texture を挟む 3 段スケーリング**: viewport (384×216) → 中間 texture (viewport × N, nearest) → window (yml サイズ, linear)

を比較し、(c) を採用する。

## Decision

**Window サイズは `bebeu-engine.yml` の `window` ブロックを source of truth とする。pixel art の整数倍境界は「viewport → 中間 render texture」段で確保し、「中間 texture → window」段だけ linear で任意倍率を許容する。**

### 3 段スケーリング

| 段 | 入力 | 出力 | 拡大手段 |
|----|------|------|---------|
| **scene → 中間** | viewport (384×216) | 中間 texture `viewport × N` (N = `floor(min(win_w/vp_w, win_h/vp_h))`, 最低 1) | sprite テクスチャは `ImagePlugin::default_nearest()` で **nearest**。整数倍なので rippling は出ない |
| **中間 → window** | 中間 texture | window 物理 pixel (yml 値) | 中間 Image の `sampler = ImageSampler::linear()` で **linear**。非整数倍を許容 |

例: viewport 384×216、window 1280×720 → N = min(1280/384, 720/216) = min(3.33, 3.33) = 3 → 中間 1152×648 → 中間→window scale = 1.111 (linear で出力)。

### Render pipeline

- [`PixelPerfectConfig`](../../packages/engine/src/app/pixel_perfect.rs) (Resource): viewport / 中間 / window の 3 段 size を起動時 entrypoint が insert
- [`PixelPerfectRenderPlugin`] の `Startup` system が:
    1. 中間 Image を `Image::new_target_texture(...)` で作り、`sampler = linear` を上書き
    2. handle を [`PixelPerfectTarget`] resource に保存
    3. Final Camera (`Camera2d` + `Camera { order: 1 }` + `RenderLayers::layer(1)`) を spawn
    4. Final Sprite (`Sprite::from_image(handle)` + `Transform::from_scale(scale)` + `RenderLayers::layer(1)`) を spawn
- 各 scene の Camera は spawn 時に `RenderTarget::Image(target.image.clone().into())` を一緒に挿入する (今は battle scene のみ camera を spawn する。title / result は camera 無しのため対応不要)

### `bebeu-engine.yml` loader

`shared::config::EngineConfig::load()` が `BEATEMUP_ENGINE_CONFIG` env > `CARGO_MANIFEST_DIR/bebeu-engine.yml` の優先順で yml を読む。`workspace_dir` と `window` の 2 フィールドを持ち、未指定は `None`。`workspace_dir` は `RuntimePaths::resolve(&EngineConfig)` で env > yml > manifest fallback の順に解決する。

`WINDOW_INTEGER_SCALE_FALLBACK = 3` は **yml.window が未指定のとき**だけ使う。dead config に戻したくないため fallback として残す。

### RenderLayers での隔離

Final Pass (Final Camera + Final Sprite) は `RenderLayers::layer(1)` に置き、scene 側 sprite (default = layer 0) と隔離する。これで:

- Battle Camera は中間 texture に layer 0 sprite だけを描く
- Final Camera は window に layer 1 (= Final Sprite だけ) を描く
- 二重描画 / 中間 texture が window に直接見えてしまう事故が起きない

### aspect 不一致時の挙動

`scale = min(win_w/inter_w, win_h/inter_h)` で計算するため、viewport と window の aspect が違うと中間 texture が aspect 維持で window 中央に配置され、上下または左右に黒帯が出る (letterbox / pillarbox)。fill (= 一部 crop) ではなく letterbox に倒すのは、絵が切れるよりは黒帯のほうが意図しない情報欠落が無いため。

## Alternatives Considered

- **(a) yml.window を直接 採用 + 整数倍ポリシー廃止**:
    - 実装が単純 (中間 texture 不要)
    - non-integer linear scaling は全体がボヤける。nearest だと rippling。pixel art の鮮鋭さを失う
- **(b) 整数倍ポリシーを保ち倍率を上げる (4× / 5×)**:
    - 鮮鋭さは保てる
    - window サイズが viewport の倍数に縛られる (1920×1080 が出せず 1920 に近いのは 1920 = 5× だが、もし viewport が 384×216 以外になるとフル HD が出せなくなる)。yml の柔軟性が失われ、dead config 問題は解決しない
- **(c) 中間 render texture (採用案)**:
    - 実装コストは「Image 1 枚 + Camera 1 個 + Sprite 1 個 + RenderLayer 隔離」と限定的
    - pixel art の中身は整数倍で完璧、エッジだけわずかにボケる (window→中間 の linear 段)。pixel-perfect ゲームでよく使われる妥協パターン (`bevy_pixel_perfect_camera` 等の crate もこの方式)
- **(d) bicubic / lanczos / HQx シェーダーを書く**:
    - 品質はさらに上がる
    - GPU の sampler は nearest / linear のみ。bicubic 以上は fragment shader 自前実装でメンテコストが付く。本プロジェクトの今のフェーズでは過剰

## Consequences

**得られたもの**

- `bebeu-engine.yml` の `window` が source of truth になり、ユーザーが yml を編集して window サイズを自由に変えられる (例: 1920×1080 でも 1280×720 でも有効)
- pixel art の中身が確実に nearest 整数倍で出る (rippling 完全に消滅)。コメントと実態の乖離 (= linear なのに nearest と書いてあった) も解消
- 「engine config の loader 未実装」という ADR-0016 が言及していた宿題を解消。`workspace_dir` も yml で指定可能に
- aspect 違い時の挙動が予測可能 (letterbox 固定)

**支払うコスト / 注意点**

- 中間 texture 経由で render pass が 1 段増える (中間 → window の Final Sprite 描画)。viewport が大きいゲームでは GPU 負荷が増える。本プロジェクトの 384×216 viewport では事実上無視できる
- Camera の `RenderTarget` 切り替えを scene ごとに行う必要がある (現状 battle のみ)。新規 scene を増やす時は `Camera2d` と一緒に `RenderTarget::Image(pixel_perfect_target.image.clone().into())` を一緒に spawn することを忘れない (= 忘れると **その scene だけ window に直書き** されて Final Pass と二重描画になる)
- Final Pass が `RenderLayers::layer(1)` を占有する。他用途で layer 1 を使いたくなったら別 layer に振り直す必要がある
- `WINDOW_INTEGER_SCALE_FALLBACK` が「yml 未指定時の保険」として残るので、yml をうっかり消したときに「window が小さくなる」現象が再発し得る (ただし dead config よりは予測可能)

**今後の拡張余地**

- editor 側で window サイズを編集 UI から変更したい場合は `EngineConfig` を editor の workspace data に migrate する (ADR-0016 が示す方向)
- aspect 違いで letterbox の代わりに fill (crop) や stretch を選べる option を yml に持たせることはできる。設計負担を払うほどの需要が出てから検討
- post-process effect (CRT / scanline / chromatic aberration 等) を入れたくなったら、Final Sprite を Material2d に差し替える形で乗せられる。中間 texture を入力とする fragment shader を書くだけで済む

## 関連

- ADR-0016: engine config のハイブリッド配置 (本 ADR で `EngineConfig` loader が実装され Stage 1 placeholder を脱する)
- [packages/engine/src/app/pixel_perfect.rs](../../packages/engine/src/app/pixel_perfect.rs): 実装
- [packages/engine/bebeu-engine.yml](../../packages/engine/bebeu-engine.yml): window 設定 (source of truth)
