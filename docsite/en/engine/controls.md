# Controls

The engine is still in the scaffolding stage and only supports keyboard input. Gamepad and camera controls are not implemented yet.

## Title scene

| Key | Action |
|---|---|
| `Enter` / `Space` | Advance to the Battle scene |

## Battle scene (Player controls)

Arrow keys and WASD are synonymous (four-way diagonal input is allowed).

### Movement

| Key | Action |
|---|---|
| `→` / `D` | Move right |
| `←` / `A` | Move left |
| `↑` / `W` | Move away from the camera (along Z) |
| `↓` / `S` | Move towards the camera (along Z) |

### Attacks

| Key | Action |
|---|---|
| `J` / `Space` | Standard attack (a standing punch) |
| `K` | Low attack — a foot-level AttackBox aimed at downed enemies in `LieDown` |
| `J` / `Space` while jumping | Air attack (JumpAttack) |

Each attack's damage / Knockback vector / Guard chip is configured per frame via `attack_box_overrides[].meta` on the Animation. See ADR-0024 and ADR-0028 for details.

### Jump and Guard

| Key | Action |
|---|---|
| `I` | Jump — ascend with `Physics.jump_velocity_y`, fall under gravity, return to `Idle` on landing. Air movement with `WASD` is allowed while jumping |
| `L` (held) | Guard — neutralises `damage` and the `knockback_gauge` while only the `guard_gauge` is consumed. Releasing the key returns to `Idle` |

Double jump and air guard are not supported (the `I` / `L` keys are ignored while in `Jump`). See ADR-0027 and ADR-0028.

### Hit reactions and combo behaviour

A hit drains `Combatant.gauge` (the Knockback gauge) by `AttackBoxMeta.knockback_damage`. When the gauge falls to `0` or below, the launch sequence fires and progresses through `KnockbackUp` → `KnockbackDown` → `Bounce` × N → `Slide` → `LieDown` → `Rise` → `Idle`, driven by the physics integrator (ADR-0024).

Two caps prevent runaway combos:

- `Physics.max_juggle_count` — maximum number of airborne re-hits. Past this limit, further airborne hits are **fully invincible** (they pass through cleanly).
- `Physics.max_down_hit_count` — maximum number of DownHit re-hits. Behaves the same way for downed targets.

When the `guard_gauge` is drained to zero while guarding, a **GuardBreak** fires: `Physics.guard_break_knockback` is loaded into the kinematic velocity and the target merges into the `KnockbackUp` flow (ADR-0028).

## HUD overlay

During the Battle scene the engine draws HP bars / rings / enemy HP bars according to the Project YAML's `hud:` section. See [editor / HUD layout](../editor/hud.md) for the element kinds and placement schema. A Project without a `hud:` section shows no HUD (treated as an empty HUD).
