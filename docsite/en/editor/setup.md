# Setup and launch

## First-time setup

```sh
just editor-desktop-setup   # alias: ed-d-setup
```

This installs the dioxus-cli (`dx`), runs `npm install` (tailwindcss / daisyui), and generates `assets/tailwind.css` in order.

## Dev server with hot reload

```sh
just editor-desktop-dev     # alias: ed-d-dev
```

This runs the tailwind `--watch` build in parallel and starts a hot-reloading dev build through `dx serve --platform desktop`.

## Open the sample project (without touching the config)

To open `sample-projects/minimal` without editing `bebeu-editor.yml`, use the dedicated recipe that sets `BEATEMUP_EDITOR_WORKSPACE_DIR`:

```sh
just editor-desktop-dev-sample   # alias: ed-d-dev-sample
```

Internally it runs `BEATEMUP_EDITOR_WORKSPACE_DIR=../../sample-projects/minimal dx serve ...`. The editor honours the env var first, so it launches immediately without consulting the YAML config or the folder picker.

## One-shot launch (no hot reload)

```sh
just editor-desktop-run     # alias: ed-d-run
```

This bypasses `dx` and launches with `cargo run -p bebeu-editor-desktop`. Only the CSS is rebuilt up front.

## Release build / distribution package

```sh
just editor-desktop-build   # alias: ed-d-build    # release build with assets bundled (target/dx/.../desktop/)
just editor-desktop-bundle  # alias: ed-d-bundle   # produce a distribution package per the [bundle] section in Dioxus.toml
```
