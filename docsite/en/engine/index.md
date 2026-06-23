# engine

A Bevy-based runtime (the `beatemup` binary). It loads a Project (`<workspace_dir>/data/projects/{name}.yml`) authored in the editor and runs it.

The workspace directory is selected with the `BEATEMUP_RUNTIME_DIR` environment variable. When unset, it looks at `packages/engine/../../runtime`, so the quickest first try is `just engine-run-sample`, which points at `sample-projects/minimal`.

## Sections

- [Launching and project selection](./run.md) — `engine-run` / `engine-run-sample` / the `--project` flag / the `BEATEMUP_PROJECT` environment variable
- [Controls](./controls.md) — keyboard controls for the Player
- [Debug build](./debug.md) — debug build, hitbox / state overlays, pause and frame advance
