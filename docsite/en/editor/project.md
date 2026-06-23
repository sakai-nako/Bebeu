# Project / Character / Level

Characters and Levels live in master pools under `<workspace_dir>/data/characters/` and `<workspace_dir>/data/levels/`. A Project at `<workspace_dir>/data/projects/{name}.yml` carries the role arrays that say "which Character / Level fills which role."

- A Project holds the `players` / `opponents` / `levels` role arrays
- The same Character / Level can be referenced from multiple Projects
- The engine reads runtime settings (window size, etc.) from `packages/engine/bebeu-engine.yml` (ADR-0016)
