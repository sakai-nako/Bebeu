# HUD layout

The in-game HUD (HP bars, HP rings, enemy HP bars) is defined under the `hud:` section of the Project YAML. Elements live in an `elements:` array, and each element carries a `kind:` discriminator (an internally-tagged enum).

In the editor, the Project detail page has a "HUD layout" card where you can add, remove, reorder, and edit elements through numeric / colour / dropdown form controls.

## Element kinds

| `kind` | Summary | Anchor | ADR |
|---|---|---|---|
| `player_hp_bar` | A horizontal / vertical bar for the Player's HP | screen | ADR-0029 / ADR-0030 |
| `player_hp_ring` | An annular sector (arc / pie) for the Player's HP | screen | ADR-0029 |
| `enemy_hp_bar` | An HP bar for an Enemy; target resolves via engagement / tag / nth | screen | ADR-0031 |
| `enemy_overhead_hp_bar` | A small bar that tracks above each Enemy entity | world | ADR-0032 |

`player_hp_bar` / `player_hp_ring` / `enemy_hp_bar` are **screen-anchored**: they position from one of the 9 screen anchors (`top_left` / `top` / `top_right` / `left` / `center` / `right` / `bottom_left` / `bottom` / `bottom_right`) plus an offset. Only `enemy_overhead_hp_bar` is **world-anchored**, tracking the Enemy entity's Transform.

## Common fields (screen-anchored kinds)

| Field | Unit / default | Role |
|---|---|---|
| `id` | string / optional | An identifier other elements can reference via `anchor_to.id` |
| `anchor` | 9 anchors / `top_left` | Base point on the screen |
| `anchor_to` | `{ id, edge }` / optional | **When set, `anchor` is ignored.** Uses another element's `edge` as the base point (ADR-0031) |
| `offset` | `{ x, "y" }` (px) | Pixel offset from the base point (X right is positive, Y down is positive) |
| `size` | `{ w, h }` (px) | The element's outer bbox |
| `frame` | `{ thickness, color }` | Outer border thickness (eats into `size` from the inside) and colour. `thickness: 0` disables the border |
| `bg_color` / `fg_color` | `#RRGGBB` / `#RRGGBBAA` | Background colour / foreground (gauge fill) colour inside the frame |
| `fill_direction` | enum / `left_to_right` | Direction the gauge depletes: `left_to_right` / `right_to_left` / `top_to_bottom` / `bottom_to_top` |

> The YAML key `y` is a YAML 1.1 truthy alias, so it **must be quoted** inside `offset` (`"y": 16.0`). The saphyr parser receives it as-is.

## `player_hp_bar`

A single bar for the Player's HP. `size` is the outer bbox; the frame and the gauge drawing area sit inside it.

The gauge depletes from the tail of `fill_direction` (in `left_to_right`, the rightmost gauge empties first). Each single gauge fills smoothly (no on/off step rendering).

Extra fields:

| Field | Default | Role |
|---|---|---|
| `target` | `p1` | The `PlayerId` shown (`p1` / `p2` / `p3` / `p4`). If the target Player is not present at spawn, the element is skipped with a warning (ADR-0030) |
| `gauge_step` | `{ fixed_count: 1 }` | Rule that splits one HP bar into multiple gauges (see below) |
| `gauge_gap` | `0.0` | Gap between gauges (px). The gap shows through to `bg_color` |

`gauge_step` is a tagged enum with two variants:

- `{ fixed_count: n }`: always split into n gauges regardless of max HP
- `{ per_unit: n }`: one gauge = n HP, so the bar splits into `ceil(max_hp / n)` gauges. The last gauge holds the remainder; visually all gauges are equal width but the last fills up earlier

```yaml
- kind: player_hp_bar
  id: p1_hp
  target: p1
  anchor: top_left
  offset: { x: 16.0, "y": 16.0 }
  size: { w: 120.0, h: 8.0 }
  frame: { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#e62626"
  fill_direction: left_to_right
  gauge_step: { fixed_count: 1 }
  gauge_gap: 0.0
```

## `player_hp_ring`

The Player's HP as an annular sector (ring or pie). `size` is the bounding bbox; radius = `min(w, h) / 2`.

| Field | Default | Role |
|---|---|---|
| `target` | `p1` | The `PlayerId` shown |
| `start_angle` | `0.0` | Start angle in degrees. 12 o'clock = 0° |
| `sweep_extent` | `360.0` | Arc angle drawn (degrees). `360` is a full ring; less is a partial arc |
| `ring_thickness` | `6.0` | Ring thickness in px. `0` becomes a pie (filled to the centre) |
| `direction` | `clockwise` | Direction the ring is drawn: `clockwise` / `counter_clockwise` |
| `gauge_step` / `gauge_gap` | same as the bar | But `gauge_gap` is in **degrees** (not px), so the visual gap is radius-independent |

`fill_direction` is not used; instead the ring depletes from the trailing segment of `direction`. With `ring_thickness < radius` the centre stays transparent, so you can overlay another HUD element (an icon, etc.) in the middle.

```yaml
- kind: player_hp_ring
  target: p1
  anchor: top_left
  offset: { x: 16.0, "y": 16.0 }
  size: { w: 48.0, h: 48.0 }
  frame: { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#e62626"
  start_angle: 15.0
  sweep_extent: 330.0
  ring_thickness: 8.0
  direction: clockwise
```

## `enemy_hp_bar`

A screen-anchored Enemy HP bar. Built to handle targets that can change over time, so in Phase 2 it is **always single-gauge** (`gauge_step` / `gauge_gap` are kept in the schema for compatibility but ignored by the engine).

`target` is an externally-tagged enum with three variants:

| YAML | Meaning |
|---|---|
| `target: { last_engaged_by: p1 }` | Shows the Enemy the given Player last hit. Switches whenever a hit lands |
| `target: { tag: boss }` | Shows the Enemy whose Character YAML has a matching `tag` field. Boss use case |
| `target: { nth_enemy: 0 }` | Shows the N-th Enemy in spawn order. Debug use |

When no Enemy matches, the entire element (frame / bg / gauge) is hidden via `Visibility::Hidden`.

```yaml
# Show the HP bar of the enemy P1 last hit, right under the "p1_hp" Player HP bar.
- kind: enemy_hp_bar
  target: { last_engaged_by: p1 }
  anchor_to:
    id: p1_hp
    edge: bottom_left
  offset: { x: 0.0, "y": 4.0 }
  size: { w: 120.0, h: 6.0 }
  frame: { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#f2d82c"
  fill_direction: left_to_right
```

To use `tag`, set a string like `tag: boss` on the Character YAML (the engine attaches it to the Enemy entity's `EnemyTag` component at spawn time).

## `enemy_overhead_hp_bar`

A world-anchored bar that tracks **above each Enemy entity** on screen. The screen-anchor fields (`anchor` / `anchor_to` / `id` / `offset.x`) are meaningless here.

| Field | Default | Role |
|---|---|---|
| `tag_filter` | omitted (all Enemies) | When set, only attaches to Enemies whose Character `tag` matches |
| `size` | `{ w: 28.0, h: 3.0 }` | Outer bbox of the bar |
| `frame` / `bg_color` / `fg_color` / `fill_direction` | same as the screen kinds | |
| `vertical_anchor` | `image_top` | Y base point (see below) |
| `offset_y` | `4.0` | Y offset from the base point (bevy Y, + is up) |

`vertical_anchor` takes one of three values:

| Value | Base point | Tracking |
|---|---|---|
| `origin` | The Enemy entity's Transform origin (= sprite anchor, usually the feet) | Static |
| `image_top` (default) | The **top edge** of the current frame's sprite | **Recomputed every frame** (the bar rises when the sprite stretches up during a jump) |
| `image_bottom` | The **bottom edge** of the current frame's sprite | **Recomputed every frame** |

```yaml
# A small yellow bar above every enemy
- kind: enemy_overhead_hp_bar
  size: { w: 28.0, h: 4.0 }
  frame: { thickness: 1.0, color: "#000000" }
  bg_color: "#00000099"
  fg_color: "#f2d82c"
  vertical_anchor: image_top
  offset_y: 4.0
  fill_direction: left_to_right
```

Spawning is per-enemy, triggered by `Added<Enemy>`. When an Enemy is despawned the bar cascades away through the Bevy hierarchy.

## Inter-element anchoring (`anchor_to`)

Screen-anchored elements may reference another element as their base point instead of `anchor`:

```yaml
- kind: player_hp_bar
  id: p1_hp                          # identifier referenced from other elements
  anchor: top_left
  offset: { x: 16.0, "y": 16.0 }
  size: { w: 120.0, h: 8.0 }
- kind: enemy_hp_bar
  target: { last_engaged_by: p1 }
  anchor_to:                          # anchor to the bottom-left of the P1 HP bar
    id: p1_hp
    edge: bottom_left
  offset: { x: 0.0, "y": 4.0 }
  size: { w: 120.0, h: 6.0 }
```

References must be **forward-only** (the parent must appear earlier in the YAML). Unresolved ids cause a warning and the element is skipped. The implementation rides on the Bevy Transform hierarchy, so if the parent follows the camera the child does too.

## Full YAML example

The HUD section of `sample-projects/minimal/data/projects/main.yml`:

```yaml
hud:
  elements:
    - kind: player_hp_bar
      id: p1_hp
      target: p1
      anchor: top_left
      offset: { x: 16.0, "y": 16.0 }
      size: { w: 120.0, h: 8.0 }
      frame: { thickness: 1.0, color: "#000000" }
      bg_color: "#00000099"
      fg_color: "#e62626"
      fill_direction: left_to_right
      gauge_step: { fixed_count: 1 }
      gauge_gap: 0.0
    - kind: enemy_hp_bar
      target: { last_engaged_by: p1 }
      anchor_to: { id: p1_hp, edge: bottom_left }
      offset: { x: 0.0, "y": 4.0 }
      size: { w: 120.0, h: 6.0 }
      frame: { thickness: 1.0, color: "#000000" }
      bg_color: "#00000099"
      fg_color: "#f2d82c"
      fill_direction: left_to_right
    - kind: enemy_overhead_hp_bar
      size: { w: 28.0, h: 4.0 }
      frame: { thickness: 1.0, color: "#000000" }
      bg_color: "#00000099"
      fg_color: "#f2d82c"
      vertical_anchor: image_top
      offset_y: 4.0
      fill_direction: left_to_right
```
