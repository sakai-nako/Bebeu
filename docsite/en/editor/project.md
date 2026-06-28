# Project / Character / Level

Characters and Levels live in master pools under `<workspace_dir>/data/characters/` and `<workspace_dir>/data/levels/`. A Project at `<workspace_dir>/data/projects/{name}.yml` defines "which Character / Level fills which role," the logical resolution, and the HUD layout.

- A Project holds the `players` / `opponents` / `levels` role arrays, a `resolution`, and a `hud:` section
- The same Character / Level can be referenced from multiple Projects
- The engine reads runtime settings (window size, etc.) from `packages/engine/bebeu-engine.yml` (ADR-0016)

## YAML fields

| Field | Unit / default | Role |
|---|---|---|
| `resolution` | `{ width, height }` (px) / `{ 640, 360 }` | Logical resolution of the drawing buffer (used as the intermediate render texture in ADR-0026) |
| `players` | list of strings / empty | Character names taking the Player role. Assigned to P1, P2, ... in order |
| `opponents` | list of strings / empty | Character names taking the Opponent role (the pool of enemies) |
| `levels` | list of strings / empty | Level names used by this Project |
| `hud` | `{ elements: [...] }` / empty | The in-game HUD layout (see [HUD layout](./hud.md)) |

The `name` field is reconstructed from the file stem, so do not write it in YAML (`#[serde(skip)]`).

## Authoring in the editor

The Project detail page (`/projects/{name}`) shows the following cards:

1. **Resolution** — width / height of the logical resolution
2. **Players / Opponents / Levels** — three multi-select widgets that build the role arrays from the Character / Level master pools
3. **HUD layout** — add / edit / reorder HUD elements (see [HUD layout](./hud.md))
4. **Launch engine** — the "Show engine launch command" button opens a modal with `just engine-run -- --project {name}` for you to copy into a terminal. The button is disabled when `players` / `opponents` / `levels` are empty

Nothing is written to disk until you Save.
