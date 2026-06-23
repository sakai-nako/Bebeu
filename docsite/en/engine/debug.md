# Debug build

`just engine-run` (alias: `en-run`) launches with the debug profile (`cargo run`). For release, use `just engine-run-release` (alias: `en-run-rel`).

```sh
just engine-run                # debug    (cargo run)
just engine-run-release        # release  (cargo run --release)
```

## Log levels

The default log filter is `wgpu=error,wgpu_core=error,wgpu_hal=error,naga=warn,info`. Because Bevy's `LogPlugin` honours `RUST_LOG` when set, that takes precedence.

```sh
RUST_LOG=debug just engine-run
```

The hit resolution path for the launch flow (ADR-0024 / ADR-0025) and the Guard path (ADR-0028) emits `tracing::info!` lines covering trigger conditions, effective vectors, and gauge values. Bumping `RUST_LOG` to `info` or `debug` is the easiest way to follow the behaviour.

## Debug overlays

Both overlays work in either the debug or release profile, and are toggled with function keys:

| Key | Overlay |
|---|---|
| `F1` | Hitbox overlay — draws AttackBox (red), BodyBox (green; invincible frames show only the outline), and Pivot points using Bevy `Gizmos` |
| `F2` | State debug overlay — renders a one-line `state / gauge / bounce / final_action / hit_from_behind / juggle / down_hit` label above each Player / Enemy entity |

Both overlays start off. Their toggle results are also logged via `tracing::info!`.

## Pause / Frame advance

For inspecting the launch flow one frame at a time:

| Key | Behaviour |
|---|---|
| `F3` | Toggle pause (halts all gameplay systems). Animation tick, hit resolution, and physics integration all stop |
| `F4` | While paused, advance one frame (single step). Any `just_pressed` inputs (e.g. `J` to attack) that were down during the pause are re-fired in that same frame |

(Implementation note: input toggle systems and overlay drawing keep running while paused. `latch_paused_input` accumulates `just_pressed` keys so that `F4` can replay them in the next stepped frame. See `features/character/debug_control.rs` for details.)

## Hitbox vs state overlay — what each is for

- **Hitbox overlay (`F1`)** — geometry-level debugging. Did the attack actually intersect? Are the invincible frames effective? Is the Pivot off?
- **State overlay (`F2`)** — semantic-level debugging. Why does the target not launch? Why no GuardBreak? Has the juggle cap kicked in? Verifiable from the gauge / counter values.
