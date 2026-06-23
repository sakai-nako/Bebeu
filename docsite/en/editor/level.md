# Level

A Level represents one stage of the Beat 'em up. A single YAML file ties together the base image (background), where the Player can walk, where enemies spawn, and where the camera starts.

## File layout

Levels live in the master pool at `<workspace_dir>/data/levels/`.

```
<workspace_dir>/data/levels/
├── ct.yml                   ← the Level itself
└── ct/
    └── base.png             ← background image (referenced by the `base` field)
```

- The YAML's filename is the Level name (do not write a `name` field inside the YAML).
- The base image is copied into the same-name directory by the editor's "import base image" action. Other filenames are allowed as long as the `base` field in the YAML names them exactly.
- The same Level can be referenced from multiple Projects (list the Level name in a Project's `levels:` array).

## YAML fields

| Field | Unit / default | Role |
|---|---|---|
| `base` | string / `"base.png"` | filename of the background image under `{level_name}/` |
| `areas` | list / 1 entry by default | walkable area (see below) |
| `camera_start_x` | image X (px) / `0` | base-image X that maps to the left edge of the camera at Level start |
| `camera_start_y` | image Y (px) / `0` | base-image Y that maps to the top edge of the camera at Level start |
| `player_spawn_x` | image X (px) / `0` | initial Player spawn X (base-image pixels; Y is always ground) |
| `player_spawn_z` | image Y (px) / `0` | initial Player spawn Z (base-image pixel Y = depth) |
| `player_respawn_y` | height (px) / `0` | starting height for respawn on death (0 = revive on the ground; positive = drop from above) |
| `opponent_triggers` | list / empty | enemy spawn schedule (see below) |

Omitted fields are filled in with their defaults automatically. An empty `opponent_triggers` can be left out of the YAML.

## Coordinate system: base-image pixel = world coordinate

Level coordinate fields use **base-image pixel positions directly**. "Place a character at this pixel of the image" becomes the world coordinate as-is, and as long as the camera is still it also matches the on-screen position.

- **X** (horizontal): base-image left → right (pixel X)
- **Z** (depth): base-image top → bottom (pixel Y). **The lower the image, the closer to the camera; the higher, the farther.** Up moves the character away (up the image), Down brings them closer (down the image).
- **Y** (height): up by jumping. Ground is Y=0.

The on-screen position is `screen_y = world_z - camera_y - world_y`. Things in front (large Z) draw lower on screen; jumping (large Y) draws higher. For X, `screen_x = world_x - camera_x`.

> The older projection parameters `ground_screen_y` / `z_scale` were dropped (the coordinate system was unified to base-image pixels). See ADR-0023 (image-pixel-world-screen-unification) for context.

## `areas` (walkable regions)

`areas` is a list of **one-side-parallel trapezoids**. Each area uses near-side Z (`near_z`, lower image → larger value) and far-side Z (`far_z`, upper image → smaller value) as the two horizontal edges; only the left and right edges may slant. Multiple entries are **OR-composed** (the Player can stand anywhere that is inside at least one area).

```yaml
areas:
  - near_z: 200.0     # near = lower image (larger value)
    far_z: 80.0       # far = upper image (smaller value)
    near_min_x: 0.0
    near_max_x: 640.0
    far_min_x: 0.0
    far_max_x: 640.0
```

Shape sketch (top-down; image top = far, image bottom = near):

```
              z = far_z   (far / upper image, smaller value)
            +-----------+
            |           |       far_min_x .. far_max_x
            |  AREA     |
           /             \
          /               \
         +-----------------+    near_min_x .. near_max_x
              z = near_z   (near / lower image, larger value)
```

- When `near_min_x == far_min_x && near_max_x == far_max_x`, the area is a rectangle.
- Separated islands or branching paths are expressed by listing two or more areas.
- Invalid values such as `near_z < far_z` (near smaller than far) or `near_min_x > near_max_x` cause an engine startup error.

For the design rationale and alternatives, see ADR-0022 (level-area-one-side-parallel-trapezoid-or).

## `opponent_triggers` (enemy spawn schedule)

When the Player's world X first crosses `trigger_x`, the trigger fires exactly once and spawns a Character named `character_name` at `(spawn_x, spawn_y, spawn_z)`.

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

- A trigger fires only once (1-shot).
- `character_name` matches a Character under `<workspace_dir>/data/characters/`. Renaming or deleting that Character does not update Level YAML automatically, so fix references manually by following the editor's warnings.

## Typical scenarios

### A. Simple rectangular Level (the default area is enough)

```yaml
base: base.png
# areas / camera_start_* / player_spawn_* / player_respawn_y may all be omitted
```

### B. Level that widens with depth (trapezoid)

```yaml
base: street.png
areas:
  - near_z: 200     # near (lower image). wide
    far_z: 140      # far (upper image). narrow
    near_min_x: 0
    near_max_x: 960
    far_min_x: 120
    far_max_x: 840
player_spawn_x: 80
player_spawn_z: 170
```

### C. Spawn enemies in sequence as the Player progresses

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

## Authoring in the editor

Detailed edits happen on the editor's Level page (`/levels/{name}`):

- Create a Level from the new-Level modal by giving it a name and a base image
- Manipulate Area / Player spawn / OpponentTrigger visually on the Canvas
- Edit numeric fields like `camera_start_*` / `player_respawn_y` from the inspector
- Nothing is written to disk until you Save (unsaved edits are protected)

You can also re-edit the YAML the editor wrote with a plain text editor. Field order and blank lines are normalised on save.
