# ADR-0006: Pivot manipulation via dedicated marker only

## Status

Accepted (2026-05-05)

## Context

`SpriteCanvas` の初期実装では、ユーザー操作が以下のように分岐していた:

- `<img>` 要素クリック → MoveSprite drag（実体は pivot を逆向きに動かす）
- HitBox overlay クリック（stop_propagation）→ MoveBox / ResizeBoxBR drag
- 中ボタン → PanCanvas

問題は `<img>` の当たり判定が広いこと。ユーザーが Box の縁付近を掴もうとして数ピクセル外すと、Pivot drag が発動してしまう。Pivot は Sprite と HitBox 全体の基準点なので、ズレると後段の Animation 表示まで影響する深刻な誤操作。

## Decision

**Pivot 操作の起点を `<img>` から「Pivot マーカー（十字 + 中央円）」に集約する**:

- `<img>` から `onmousedown` を外す。クリックしても何も起きない（canvas root に bubble して `selected_box` クリアだけ）
- Pivot マーカーを `pointer-events-auto` + `cursor-grab/grabbing` + `onmousedown` でインタラクティブ化
- マーカーサイズを 16×16 → 28×28 に拡大（高解像度環境での視認性 + 当たり判定の確保）

ハンドラ名は意図に合わせて `on_sprite_mousedown` → `on_pivot_mousedown` にリネーム。

## Alternatives Considered

- **Property panel に「Lock Pivot」トグル**: 追加 UI が必要 / ユーザーが切替を意識する必要がある / 「ロック忘れ」で誤操作は依然発生しうる。
- **修飾キー（Alt+drag で pivot）**: 発見性が低い（覚えるまでハマる）。マウス系操作で modifier はミスタイプを誘発しやすい。
- **HitBox の当たり判定を広げ、画像の有効クリック領域を狭める**: 「狭くした分の余白」が依然 pivot drag 領域として残るので問題は緩和されるだけで解決しない。

## Consequences

- Pivot 誤操作は **構造的に発生しない**（Pivot マーカーを掴まない限り pivot は動かない）。
- 「ここが pivot を動かす点」がマーカーとして可視化されるので、暗黙の image-drag より発見性が高い。
- マーカーが 28×28 なので canvas を少しだけ占有する。pivot 近くの小さな Box が掴みにくいケースが将来出るかもしれない。その時は (a) マーカーをドラッグ中だけ縮小、(b) 修飾キーで Box を優先、などの対応余地あり。
- `<img>` の `draggable: false` は引き続き必要（ネイティブドラッグが mousemove を奪うため）。
