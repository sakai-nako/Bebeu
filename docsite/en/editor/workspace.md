# Selecting a workspace

The editor decides which workspace directory to open via the `workspace_dir` key in `bebeu-editor.yml`. Where it reads that file depends on the build profile:

- **debug build**: `bebeu-editor.yml` in the current working directory (the `editor-desktop-*` recipes in the justfile launch with `packages/editor-desktop/` as the CWD)
- **release build**: `bebeu-editor.yml` next to the executable

If no config file is found, a folder picker appears at launch and a new file is written to the chosen directory.

```yaml
# packages/editor-desktop/bebeu-editor.yml (default shipped with the repo)
workspace_dir: ../../sample-projects/minimal
```

Workspace directory layout:

```
<workspace_dir>/
└── data/
    ├── characters/   ← master pool (shared across all Projects)
    ├── levels/       ← master pool (shared across all Projects)
    └── projects/     ← per-Project YAML
```

By default `sample-projects/minimal` works as the workspace directory directly (a minimal project built from CC0 placeholder assets). To start your own project, copy `sample-projects/minimal` somewhere outside the repo and point `workspace_dir` at the new path.
