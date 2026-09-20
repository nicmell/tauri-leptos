# Architecture

A pure-Rust application template: **Leptos SSR** rendered by an **axum**
server, wrapped by the **Tauri 2** shell. One origin in production —
page, API, and WebSocket from a single server; the standard Tauri
dev/build flow drives everything.

## Crate map

```
crates/ui        Leptos app. lib (feature hydrate): the wasm client.
                 bin  (feature ssr): the dev frontend-server run by
                 `cargo leptos watch`. server.rs: leptos_options,
                 leptos_router(options, api_base), production router().
crates/app-core  config + paths + logging + the API router (axum:
                 /api/hello, /api/counter, /ws) and bind/serve/shutdown.
crates/app-cli   tauri-leptos-cli: full single-origin server by default
                 (SSR + api, the systemd deployment), --headless = api
                 only (the dev split); plus `config write|validate`.
src-tauri        Tauri shell. Feature ssr = in-process production server.
```

## Dev: two processes, state stays alive

```
cargo leptos watch ──► ui bin :3000  SSR + hydration (hot reload)
                                     injects <meta name="api-base"
                                     content="http://127.0.0.1:3001">
tauri-leptos-cli --headless ──► api :3001  axum only, holds state, CORS
                                     the local dev origins
```

- UI edits rebuild only the frontend-server; the API process — and its
  in-memory state — survives (`/api/counter` proves it).
- `mprocs` starts the pair for browser work; `cargo tauri dev` spawns
  its own watch (`beforeDevCommand`) and opens the window on it.
- The client reads the `api-base` meta and sends fetch/WS there,
  cross-origin. CORS lives only on the cli and only for the dev
  origins (`http://127.0.0.1:3000`, `tauri://localhost`,
  `http://tauri.localhost`).
- **Server functions run in the frontend-server process in dev.** Keep
  them stateless; real state belongs behind `/api` and `/ws`.

## Prod: one server, one origin

`cargo tauri build -f ssr`: the shell starts the merged router
(`ui::server::router` = leptos routes + core API) in-process on
`127.0.0.1:3000` and the window — a plain config window with a static
`url` — loads it. The `api-base` meta is empty, so the client uses
relative URLs: no CORS, no second port. The site comes from the
bundled resources (`bundle.resources` → `resource_dir()/site`), with
the workspace `target/site` as fallback for unbundled runs.

The frontend origin (`127.0.0.1:3000`) is fixed by design and never
configurable.

## Asset naming and site builds

- `.cargo/config.toml` pins `LEPTOS_OUTPUT_NAME`: leptos derives asset
  URLs (e.g. `pkg/tauri-leptos.wasm`) from it at compile time, and only
  cargo-leptos builds set it on their own — without the pin, plain
  cargo builds (the shell with `-f ssr`) would emit wasm-bindgen's
  `_bg.wasm` name and hydration would 404.
- **Dev cargo-leptos builds instrument the markup for hot reload** in a
  way only their own watch process can hydrate against. Any server
  built with plain cargo must serve a **release** site
  (`cargo leptos build --release`, guaranteed by `beforeBuildCommand`).
- `hash-files` stays off (stable asset names).

## Config

`config.toml` in the app config dir, seeded with defaults on first run:

| field | default | meaning |
| --- | --- | --- |
| `api_addr` | `127.0.0.1:3001` | API/WS server address |
| `log_to_file` | `false` | daily-rolling file in the app log dir |

The cli fails fast on an invalid file (systemd must see it); unknown
fields are rejected. `serve --listen` overrides `api_addr`.

## Paths

`AppPaths` (app-core) mirrors the Tauri v2 path API names; standalone
resolution matches Tauri's per-platform mapping (macOS
`~/Library/Application Support/<id>`, Linux XDG). Overrides:
`--app-dir` / `TAURI_LEPTOS_APP_DIR` (empty = unset; everything under
one folder — `appdir/` for reproducible dev runs) > systemd
`*_DIRECTORY` env vars > platform defaults.

## Logging

One tracing stack; the writer changes per context: stderr (journald
under systemd), opt-in rolling file (`log_to_file`), logcat on Android,
browser console for the wasm side. `RUST_LOG` overrides the `info`
default.

## Dev workflows

```bash
mprocs                       # watch (:3000) + api (:3001)
cargo tauri dev              # desktop: spawns the watch, window on :3000
cargo tauri build -f ssr     # production bundle (merged in-process server)
cargo leptos build --release # site for plain-cargo servers
```

## Headless / Raspberry Pi

`tauri-leptos-cli serve` (no flags) runs the same merged single-origin
server standalone: site from `--site-root` (deb: `/usr/share/tauri-leptos/site`;
manual Linux default `/usr/local/share/tauri-leptos/site`; elsewhere
`target/site`). `--headless` keeps the api-only dev behavior. Packaging:
`scripts/build-deb.sh` → deb with binary+site+system unit (enable/start
on install). See [install/raspberry-pi.md](install/raspberry-pi.md).

## Out of scope for now

Android (`src-tauri/gen/` predates this flow),
e2e testing.
