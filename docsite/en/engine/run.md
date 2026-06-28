# Launching and project selection

## Launching

```sh
just engine-run                  # alias: en-run         # debug
just engine-run-release          # alias: en-run-rel     # release
```

Under the hood: `cargo run -p bebeu-engine --bin bebeu-engine` (with `--release` for the release build).

## Run with the sample project

If you do not have a private workspace at `runtime/`, pass the CC0 placeholder project at `sample-projects/minimal` via `BEATEMUP_RUNTIME_DIR`:

```sh
just engine-run-sample           # alias: en-run-sample
```

Internally this runs `BEATEMUP_RUNTIME_DIR=../../sample-projects/minimal cargo run ...`. The placeholder PNGs can be regenerated up front with `just gen-sample`.

## Selecting a project

```sh
just engine-run -- --project my-project
just engine-run-sample -- --project=minimal
```

The `--project=<name>` flag (or `--project <name>`) selects `<workspace_dir>/data/projects/{name}.yml`. Without the flag, the engine falls back to the `BEATEMUP_PROJECT` environment variable. If neither is set, project loading is skipped and only the Title scene comes up.

## Build

```sh
just engine-build                # alias: en-build
```

Runs `cargo build --release` and then copies `target/release/bebeu-engine(.exe)` into `runtime/build/`.
