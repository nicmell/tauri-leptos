# Architecture

A pure-Rust application template: **Leptos SSR** rendered by an **axum**
server, wrapped by the **Tauri 2** shell. One origin in production —
page, API, and WebSocket from a single server; the standard Tauri
dev/build flow drives everything.

## Crate map

```
crates/ui        Leptos app. lib (feature hydrate): the wasm client.
                 bin  (feature ssr): the dev server run by
                 `cargo leptos watch`. server.rs: leptos_options,
                 leptos_router(options, assets), merged router().
crates/app-core  config + paths + logging + the API router (axum:
                 /api/hello, /api/counter, /ws) and bind/serve/shutdown.
crates/app-cli   tauri-leptos-cli: thin wrapper around the merged
                 single-origin router (`--host`/`--port`/`--site-root`);
                 plus `config write|validate`.
src-tauri        Tauri shell. Feature ssr = in-process production server.
```

## One router, one port — everywhere

Every mode mounts the same merged router (`ui::server::router` =
leptos routes + core API + `/ws`) on a single origin. The client always
uses relative URLs: no CORS anywhere, no second port, no injected
API address.

```
dev      cargo leptos watch ──► ui bin :3000   (hot reload)
cli      tauri-leptos-cli serve ──► --host/--port (default 127.0.0.1:3000)
tauri    in-process server, window loads it
```

Dev notes:

- `view!`/CSS edits hot-patch without restarting; edits to Rust logic
  rebuild and **restart the dev server process**, so in-memory api
  state resets — the same thing a redeploy does. Real state belongs in
  an external store, not in process memory.
- `cargo tauri dev` spawns its own watch (`beforeDevCommand`) and opens
  the window on it.
- **Server functions stay stateless by convention**; state lives behind
  `/api` and `/ws` in `core::server::api_router`.

## Prod: the same router, in-process, ephemeral port

`cargo tauri build -f ssr`: the shell binds the merged router
in-process on `127.0.0.1:0` and creates the window at runtime
(`WebviewWindowBuilder` in setup) on the real bound address — no fixed
port can ever conflict with something else on the user's machine. The
site comes from the bundled resources (`bundle.resources` →
`resource_dir()/site`), with the workspace `target/site` as fallback
for unbundled runs.

The fixed `127.0.0.1:3000` remains only where an anchor is needed:
the dev watch (devUrl, adb reverse) and the cli default.

## Unified asset serving (SiteAssets)

Every mode serves site assets through one interface —
`ui::server::SiteAssets` (`open(rel) -> std::fs::File`), plugged into
the router's fallback (stream + mime on hit, SSR shell on miss):

| impl | used by | resolution |
| --- | --- | --- |
| `DirAssets(dir)` | cli `--site-root`, dev frontend-server, tests | plain filesystem + traversal guard |
| `TauriAssets` (src-tauri) | desktop bundle AND Android, same code | `resource_dir()/site` via the fs plugin (desktop: real files; Android: APK assets as file descriptors) |

On Android there is no extraction: assets are opened from the APK per
request (compressed assets are cache-copied by the plugin — correct;
`noCompress` would yield raw-APK fds). The server runs unconditionally
there (`any(feature ssr, target_os android)`).

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
| `listen` | `127.0.0.1:3000` | server bind address (SSR + api + ws) |
| `log_to_file` | `false` | daily-rolling file in the app log dir |

The cli fails fast on an invalid file (systemd must see it); unknown
fields are rejected. `serve --host`/`--port` override `listen`.

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
cargo leptos watch           # browser dev: everything on :3000
cargo tauri dev              # desktop: spawns the watch, window on :3000
cargo tauri build -f ssr     # production bundle (merged in-process server)
cargo leptos build --release # site for plain-cargo servers
```

## Standalone / Raspberry Pi

`tauri-leptos-cli serve` (no flags) runs the same merged single-origin
server standalone: site from `--site-root` (deb: `/usr/share/tauri-leptos/site`;
manual Linux default `/usr/local/share/tauri-leptos/site`; elsewhere
`target/site`); bind from `--host`/`--port` (default `127.0.0.1:3000` —
the Pi unit passes `--host 0.0.0.0`). Packaging: `scripts/build-deb.sh`
→ deb with binary+site+system unit (enable/start on install). See
[install/raspberry-pi.md](install/raspberry-pi.md).

## Out of scope for now

e2e testing; iOS (never started — would follow the desktop path,
bundle resources are real files there).
