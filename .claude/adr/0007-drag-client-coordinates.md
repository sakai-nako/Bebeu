# ADR-0007: Drag tracking uses `client_coordinates`

## Status

Accepted (2026-05-05)

## Context

`SpriteCanvas` の初期実装は `MouseEvent::element_coordinates()`（ブラウザの `offsetX/Y`）でマウス位置を取得していた。`offsetX/Y` は **イベントターゲット相対**（カーソル下にある DOM 要素の padding edge を原点とする）。

drag 中、カーソルは canvas 内を: `<img>` → `HitBoxOverlay` → canvas root → 別 HitBox … と跨ぐ。target が変わるたびに `offsetX/Y` の参照系が静かに切り替わる。症状として「drag を始めた瞬間に sprite が右下に飛ぶ」という再現性の高いバグが出た。

ワークアラウンド試行: 「最初の `mousemove` の offset を origin にし、以降は delta で扱う」。だが 2 回目以降の mousemove も target が違えば参照系が違うため、結局壊れる。

## Decision

drag 開始位置を `MouseEvent::client_coordinates()`（= `clientX/Y`、ビューポート相対、target 非依存）で取得する:

- `mousedown` 時点で `start_mouse: [i32; 2]` を確定（"first mousemove で初期化" のトリックを廃止）
- `mousemove` も `client_coordinates()` で読む
- delta = `current - start_mouse` は CSS pixel（zoom 適用前）
- image-pixel に変換するときだけ zoom で割る（`delta_zoomed`）

実装は `widgets/character/ui/sprite_canvas.rs::client_xy`。

## Alternatives Considered

- **`page_coordinates`（`pageX/Y`）**: target 非依存という意味では同じだが、document scroll の影響を受ける。canvas が scroll する設計には今のところしないが、`client` のほうが意図が狭くて事故が少ない。
- **`screen_coordinates`**: マルチモニタ + DPI scaling で意外な値が混じる。座標系として広すぎる。
- **`element_coordinates` + listener element の rect 計算**: `getBoundingClientRect()` 相当を Rust 側で持つ必要があり、transform をかけている canvas では実装が脆い。

## Consequences

- drag 計算が単純（CSS pixel delta → zoom で割って image-pixel）。
- `<img>` のネイティブ drag-and-drop を無効化（`draggable: false`）する必要がある。HTML5 drag が mousedown 直後に起動すると mousemove が drag イベントに置き換わって我々のハンドラに届かなくなるため。
- pan も同じ仕組みで delta を加算する（zoom で割らないだけ）。
- "first mousemove で start を初期化する" トリックが不要になり、`DragState::start_mouse` は `Option<[i32; 2]>` ではなく `[i32; 2]` の確定値になった。
