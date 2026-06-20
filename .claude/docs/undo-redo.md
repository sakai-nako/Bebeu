# Undo/Redo: `History<T>` / `UseHistory<T>` リファレンス

editor の Undo/Redo は `shared/history.rs` の `History<T>` を `Signal<T>` ベースの hook で包んだもの。設計の経緯は ADR-0010 を参照。ここでは **使い方** と **呼び出し規約** をまとめる。

## API 概要

```rust
use crate::shared::{use_history, UseHistory};

let draft = use_signal(|| sprite_group.clone());
let history = use_history(draft, capacity);   // UseHistory<SpriteGroup>
```

`UseHistory<T>` は `Copy + 'static`。`#[component]` の props や onclick / `use_effect` / `use_keyboard_action` のクロージャに自由に渡せる。クロージャ内で mut メソッドを呼ぶときは local rebind が必要:

```rust
onclick: move |_| {
    let mut h = history;   // Copy なのでローカル変数として再束縛
    h.undo();
}
```

提供メソッド:

| メソッド | 役割 |
|---|---|
| `record(&mut self)` | 現在の `target.peek()` を past に積む。future はクリア（新しい分岐） |
| `undo(&mut self)` | past から 1 件 pop し `target.set(prev)`。現在値は future 側に積む |
| `redo(&mut self)` | future から 1 件 pop し `target.set(next)` |
| `can_undo(&self) -> bool` | past が空でない |
| `can_redo(&self) -> bool` | future が空でない |

## 呼び出し規約

### 1. `record()` は mutation **直前** に呼ぶ

`record()` は `target.peek().clone()` をスナップショットとして past に積むので、**`draft.set(...)` より前**に呼ぶ。順序を逆にすると past に「変更後の値」が入ってしまい Undo が空回りする。

```rust
// OK
let mut updated = draft();
updated.sprites[i].pivot_point[0] += 1;
history.record();    // ← まず past に「変更前の draft」を積む
draft.set(updated);  // ← その後で更新

// NG
draft.set(updated);
history.record();    // 既に変更後の値を積んでしまう
```

### 2. 値が変わらない時は `record()` を呼ばない

数値 input の onchange など、同値で再発火し得る経路では事前にガードする（履歴を無駄に消費しない）。

```rust
if s.pivot_point[axis] == v {
    return;   // 値が同じなら no-op
}
s.pivot_point[axis] = v;
history.record();
draft.set(updated);
```

参考: `widgets/character/ui/sprite_property_panel.rs`。

### 3. drag は **mousedown で 1 回だけ** record する

mousemove で record すると 1 ピクセル動かすたびに履歴が積まれて capacity を瞬殺する。drag 開始時点（mousedown）に 1 回だけ呼び、その後の mousemove は draft を直接書き換える。空 drag（mousedown → 同位置で mouseup）の場合は同値が past に積まれるが、Undo しても見かけ変わらないだけで実害はない。

参考: `widgets/character/ui/sprite_canvas.rs:244` (Pivot)、`:386` (MoveBox)、`:405` (ResizeBox)。

### 4. Save では history を触らない

保存（`SpriteGroupEditorActions::on_save`）は disk に書くだけで history はそのまま。「保存したけどやっぱり戻したい」を許容する設計（→ ADR-0010）。保存後の Undo は disk と画面が一時的に乖離するが、再 Save で追従する。

## 配線パターン: editor 内での propagate

`SpriteGroupEditor`（widgets 層）が hook を初期化し、`UseHistory<SpriteGroup>` を子に props で配る。

```text
SpriteGroupEditor (widgets)
├── use_signal(|| sprite_group.clone())  → draft
├── use_history(draft, capacity)         → history  ← ここで生成
│
├── SpriteGroupEditorActions (features)  ← history を渡す
│   ├── on_add_box           → history.record()
│   ├── move_pivot           → history.record()
│   └── use_keyboard_action(Action::UndoSpriteGroup, || history.undo())
│
├── SpriteCanvas (widgets)               ← history を渡す
│   ├── on_pivot_mousedown   → history.record()
│   └── HitBoxOverlay        ← history を再 propagate
│       ├── on_box_mousedown    → history.record()
│       └── on_handle_mousedown → history.record()
│
└── SpritePropertyPanel (widgets)        ← history を渡す
    ├── on_pivot (number input) → history.record()
    └── on_box_field (resize)   → history.record()
```

features 層から `ApplyFirstSpriteButton` が history を受け、Modal 内で 1 回 `record()` してから一括反映する例もある（`features/character/ui/apply_first_sprite_to_others.rs`）。

## Capacity（履歴上限）

- 実体は `Preferences::sprite_group_history_capacity: u32`（デフォルト 50）
- `SpriteGroupEditor` が `use_preferences().peek().sprite_group_history_capacity as usize` で読み取り、`use_history` に渡す
- `peek()` で読む = subscribe しないので、editor を開いている間 preferences を変えても history は再生成されない（次回 editor を開くと反映）
- `History::new(capacity.max(1))` で底打ちされているので、preferences が 0 でも履歴は最低 1 件は持てる

UI 上で capacity を変更する features は `features/preference/ui/edit_history_capacity.rs`。

## キーボードショートカット

`Action::UndoSpriteGroup` / `Action::RedoSpriteGroup`（ADR-0009）で発火する。`SpriteGroupEditorActions` が `use_keyboard_action` で受けて `history.undo()` / `history.redo()` を呼ぶ。

```rust
use_keyboard_action(Action::UndoSpriteGroup, move || {
    let mut h = history;   // Copy なので毎回ローカル束縛
    h.undo();
});
```

## 関連

- ADR-0010: Undo/Redo を editor session-scope の snapshot 履歴で実装する
- ADR-0009: Keyboard shortcut dispatch（Undo / Redo Action もここに乗る）
- `widgets/character/ui/README.md`: SpriteGroupEditor の状態所有関係
- `entities/preference/README.md`: Preferences 構造、`peek()` 共有パターン
