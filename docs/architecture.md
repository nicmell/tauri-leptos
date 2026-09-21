# Architecture

A pure-Rust application template: **Leptos SSR** rendered by an **axum**
server, wrapped by the **Tauri 2** shell. One origin in production —
page, API, and WebSocket from a single server; the standard Tauri
dev/build flow drives everything.

## Crate map

```
crates/ui        Leptos app. lib (feature hydrate): the wasm client.
                 bin  (feature ssr): the watch server run by
                 `cargo leptos watch` (:3001, internal). server.rs:
                 leptos_options, leptos_router(options, assets), router().
crates/app-core  config + paths + logging + the API router (axum:
                 /api/hello, /api/counter, /ws), bind/serve/shutdown,
                 and the asset backends (assets::Assets).
crates/app-cli   tauri-leptos-cli: thin wrapper around the single-origin
                 router; build features pick what it serves.
src-tauri        Tauri shell: in-process server on an ephemeral port,
                 always; build features pick what it serves.
```

## One origin, feature-picked content

Every mode serves a single origin; the build features of the cli and
the shell pick what lives behind it:

| features | serves |
| --- | --- |
| `site` (default) | embedded SSR frontend + api |
| `dev` | api locally, pages/assets reverse-proxied from the watch |
| neither (cli only) | api only — a remote api server |

```
dev      cargo leptos watch ──► watch server :3001 (SSR, hot reload, internal)
         cli/shell dev build ──► api + reverse proxy to :3001
         browser entry :3000 (cli) · tauri window: ephemeral port
cli      tauri-leptos-cli serve ──► --host/--port (default 127.0.0.1:3000)
tauri    in-process server on an ephemeral port, window created on it
```

Dev notes:

- The dev split is invisible to the client: one origin, relative URLs.
  The api process (cli or shell) **survives frontend rebuilds** —
  `/api/counter` proves it. Pages are rendered by the watch (dev
  hot-reload instrumentation only hydrates against its own process),
  server-fn POSTs pass through the reverse proxy.
- `view!`/CSS edits hot-patch in place; edits to Rust logic rebuild
  only the watch server.
- `cargo tauri dev -- --no-default-features --features dev` spawns the
  watch (`beforeDevCommand`) and proxies to it; runner args pass the
  feature set through to cargo.
- **Server functions stay stateless by convention**; state lives behind
  `/api` and `/ws` in `core::server::api_router`.

## Prod: the same router, in-process, ephemeral port

`cargo tauri build`: the shell binds the merged router
in-process on `127.0.0.1:0` and creates the window at runtime
(`WebviewWindowBuilder` in setup) on the real bound address — no fixed
port can ever conflict with something else on the user's machine. The
site comes from the bundled resources (`bundle.resources` →
`resource_dir()/site`), with the workspace `target/site` as fallback
for unbundled runs.

The fixed `127.0.0.1:3000` remains only where an anchor is needed:
the dev watch (devUrl, adb reverse) and the cli default.

## Asset backends (core::assets::Assets)

Site serving goes through one interface —
`assets::Assets { into_router(self, on_miss) }` — where `on_miss`
renders the SSR shell in full builds:

| impl | used by | resolution |
| --- | --- | --- |
| `DirAssets(dir)` (core) | cli `site` builds, watch server, tests | `tower_http::ServeDir` (traversal guard, ETag, ranges) |
| `ProxyAssets(url)` (core, feature `dev`) | cli/shell dev builds | reverse proxy to the watch (`axum-reverse-proxy`) |
| `TauriAssets` (src-tauri) | desktop bundle AND Android, same code | `resource_dir()/site` via the fs plugin (desktop: real files; Android: APK assets as file descriptors) |

On Android there is no extraction: assets are opened from the APK per
request (compressed assets are cache-copied by the plugin — correct;
`noCompress` would yield raw-APK fds). The shell's only compile-time branch is `cfg(dev)`: dev builds
attach to the watch, everything else embeds the server.

## Asset naming and site builds

- `.cargo/config.toml` pins `LEPTOS_OUTPUT_NAME`: leptos derives asset
  URLs (e.g. `pkg/tauri-leptos.wasm`) from it at compile time, and only
  cargo-leptos builds set it on their own — without the pin, plain
  cargo builds (the shell) would emit wasm-bindgen's
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
| `listen` | `127.0.0.1:3000` | server bind address |
| `site_root` | platform default | frontend bundle dir (`site` builds) |
| `log_to_file` | `false` | daily-rolling file in the app log dir |
| `dev.upstream` | `http://127.0.0.1:3001` | watch server the dev proxy targets |

The cli fails fast on an invalid file (systemd must see it); unknown
fields are rejected. `serve --host`/`--port` override `listen`.
`dev.upstream` must match `site-addr` in the leptos metadata and
`devUrl` in tauri.conf.json — three places, kept aligned by hand.

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
cargo leptos watch           # the frontend half (:3001, internal)
cargo run -p tauri-leptos-cli --no-default-features --features dev -- serve
                             # the api half; browser entry on :3000
cargo tauri dev -- --no-default-features --features dev
                             # desktop: spawns the watch, ephemeral window
cargo tauri build            # production bundle (embedded frontend)
cargo leptos build --release # site for plain-cargo servers
```

## Standalone / Raspberry Pi

`tauri-leptos-cli serve` (no flags) runs the single-origin server
standalone: site from `site_root` in the config (Linux default
`/usr/share/tauri-leptos/site` — the deb path; elsewhere
`target/site`); bind from `--host`/`--port` (default `127.0.0.1:3000` —
the Pi unit passes `--host 0.0.0.0`). Build without default features
for an api-only server (remote frontends). Packaging:
`scripts/build-deb.sh` → deb with binary+site+system unit
(enable/start on install). See
[install/raspberry-pi.md](install/raspberry-pi.md).

## Out of scope for now

e2e testing; iOS (never started — would follow the desktop path,
bundle resources are real files there).
