# Architecture

A Tauri 2 + Leptos template that runs the same application three ways:
desktop app, Android app, and headless server. One HTTP server serves
every mode; the Tauri shells are thin windows onto it.

## Crate map

```
crates/ui        Leptos frontend (CSR wasm, built by Trunk into dist/)
crates/app-core  paths + logging + the axum server; no Tauri dependency
crates/app-cli   `tauri-leptos-cli` headless binary; no Tauri dependency
src-tauri        Tauri shell: desktop binary + Android entry point
                 (depends on app-core)
```

`src-tauri` and `app-cli` are the two executables; both compose the same
`app-core`. The headless binary never links Tauri (or webkit), which is
what makes it trivially cross-compilable for the Raspberry Pi.

## Run modes

All three modes run the same axum server from `app-core::server`:

| mode | entry | server bind | UI |
| --- | --- | --- | --- |
| desktop | `src-tauri` binary | `127.0.0.1:<ephemeral>` | webview pointed at the server URL |
| android | `mobile_entry_point` in `src-tauri` lib | `127.0.0.1:<ephemeral>` | webview pointed at the server URL |
| serve | `tauri-leptos-cli serve` | `--listen` (default `127.0.0.1:3000`) | any browser |

Single origin by construction: assets, `/api`, and `/ws` come from the
same server, so there is no CORS, no Tauri IPC (`invoke`), and no
injected base URL. The webview is created at runtime from the window
config (`create: false` in `tauri.conf.json`) with
`WebviewUrl::External` carrying the ephemeral port.

## HTTP surface

- `GET /api/hello?name=…` — sample JSON endpoint
- `GET /ws` — WebSocket echo (Text/Binary frames)
- everything else — the embedded frontend bundle, with SPA fallback to
  `index.html`; `503` with a hint until `trunk build` has run

## Asset embedding

`app-core` embeds the repo-root `dist/` (Trunk's output) with
`rust-embed`:

- **release**: files compiled into the binary — single self-contained
  executable
- **debug (desktop/serve)**: read from disk at runtime — edit UI, run
  `trunk build`, reload; no Rust rebuild
- **android**: `debug-embed` forces embedding even in debug builds (the
  host path does not exist on the device)

Build order is always `trunk build` → `cargo build`; the Tauri
`beforeDevCommand`/`beforeBuildCommand` hooks do this automatically.

## Paths

`app-core::paths::AppPaths` uses the Tauri v2 path API names. In tauri
mode the shell reads Tauri's own `PathResolver`; standalone mode
reproduces the same mapping:

| name | macOS | Linux |
| --- | --- | --- |
| `app_config_dir` | `~/Library/Application Support/<id>` | `~/.config/<id>` |
| `app_data_dir` | `~/Library/Application Support/<id>` | `~/.local/share/<id>` |
| `app_local_data_dir` | `~/Library/Application Support/<id>` | `~/.local/share/<id>` |
| `app_cache_dir` | `~/Library/Caches/<id>` | `~/.cache/<id>` |
| `app_log_dir` | `~/Library/Logs/<id>` | `~/.local/share/<id>/logs` |

`<id>` = `com.nick.tauri-leptos`.

Standalone override precedence (highest first):

1. `--app-dir <DIR>` / `TAURI_LEPTOS_APP_DIR` (empty value = unset) —
   everything under one folder (`config/ data/ cache/ logs/`);
   reproducible dev and test runs
2. systemd directory env vars (`CONFIGURATION_DIRECTORY`,
   `STATE_DIRECTORY`, `CACHE_DIRECTORY`, `LOGS_DIRECTORY`), as injected
   by the corresponding unit directives
3. the platform defaults above

## Logging

One `tracing` stack everywhere; only the writer changes:

| context | writer |
| --- | --- |
| desktop + serve | stderr (journald under systemd) |
| serve with `--log-to-file` | + daily-rolling file in `app_log_dir` |
| android | logcat (tag `tauri-leptos`, via paranoid-android) |
| browser/wasm | console (tracing-web) |

`RUST_LOG` overrides the default `info` filter.

## CLI

```
tauri-leptos-cli serve [--listen <ADDR:PORT>] [--log-to-file] [--app-dir <DIR>]
```

## Dev workflows

- **UI iteration (browser, hot reload)**: terminal A
  `cargo run -p tauri-leptos-cli -- serve`, terminal B `trunk serve` →
  browse `:1420`; Trunk proxies `/api` and `/ws` to `:3000` so the
  browser stays same-origin.
- **Desktop**: `cargo tauri dev`. After a UI edit:
  `trunk build --config Trunk.toml` + reload the window (debug builds
  read `dist/` from disk).
- **Headless**: `cargo run -p tauri-leptos-cli -- serve`.
