# Architecture

A pure-Rust application template: **Leptos SSR** rendered by an **axum**
server, wrapped by the **Tauri 2** shell. One origin in production —
page, API, and WebSocket from a single server; the standard Tauri
dev/build flow drives everything.

## Crate map

```
crates/ui        Leptos frontend, a pure library with no core
                 dependency: lib (feature hydrate) is the wasm client;
                 server.rs (feature ssr) is just router(addr, api_base)
                 — SSR routes + server fns, 404 on unknown paths.
crates/app-core  everything else: config + paths + logging + the API
                 router (/api/hello, /api/counter, /ws), the private
                 asset backends, and the ONE router — Ctx::router,
                 inferred from the Host fixed at bootstrap (core
                 depends on ui).
crates/app-cli   tauri-leptos-cli: thin wrapper around the single-origin
                 router; build features pick what it serves.
src-tauri        Tauri shell: in-process server on an ephemeral port,
                 always; build features pick what it serves.
```

## One origin everywhere

Every mode serves a single origin, and no app crate has cargo
features: core depends on ui and owns the ONE router — `Ctx::router`,
inferred from the `Host` fixed at bootstrap (`resolve` = standalone,
`from_tauri(app, cfg!(dev))` = shell; the mode is runtime data, never
a compile branch in an entrypoint). The cli is always the full server
(a remote api server is the same binary with `cors_origins` set):

```
dev      cargo leptos watch ──► runs the cli (site build) on :3001
                                (appdir config; SSR + api, hot reload)
         cargo tauri dev    ──► shell: api in-process + reverse proxy to :3001
                                window on the ephemeral origin (a plain
                                http server — open it in a browser too)
cli      tauri-leptos-cli serve ──► --host/--port (default 127.0.0.1:3000)
tauri    in-process server on an ephemeral port, window created on it
```

Dev notes:

- The dev split is invisible to the client: one origin, relative URLs.
  The shell process — and its api state — **survives frontend
  rebuilds** (POST `/api/counter` proves it). Pages are rendered by the
  watch (dev hot-reload instrumentation only hydrates against its own
  process), server-fn POSTs pass through the reverse proxy.
- `view!`/CSS edits hot-patch in place; edits to Rust logic rebuild
  only the watch server.
- `cargo tauri dev` spawns the watch (`beforeDevCommand`) and proxies
  to it.
- **Server functions stay stateless by convention**; state lives behind
  `/api` and `/ws` in `core::server::api_router`.

## Prod: the same router, in-process, ephemeral port

`cargo tauri build`: the shell binds the merged router
in-process on `127.0.0.1:0` and creates the window at runtime
(`WebviewWindowBuilder` in setup) on the real bound address — no fixed
port can ever conflict with something else on the user's machine. The
site comes from the bundled resources (`bundle.resources` →
`resource_dir()/site`); an unbundled `cargo run -p tauri-leptos` has
no resources — point `site_root` in the config at a built site if you
need that mode.

The fixed `127.0.0.1:3000` remains only where an anchor is needed:
the dev watch (devUrl, adb reverse) and the cli default.

## Remote api (`api_base`)

The frontend and the api can live on different hosts: whoever renders
the SSR page injects its configured `api_base` into the
`<meta name="api-base">` (always present; empty = same origin, the
default), and the client sends fetch/WS there. **Server functions are
NOT covered**: they always call the origin that rendered the page —
they are part of the frontend server, not of the remote api. Put
remote-capable logic behind `/api`, keep server functions for
page-local concerns. To make the contract visible in the URL — and
route shadowing impossible — server functions live under **`/fn`**
(`server-fn-prefix` in the leptos metadata + the `SERVER_FN_PREFIX`
pin in `.cargo/config.toml`, which must match). The api host then
needs `cors_origins` covering the frontend's origin — the tauri
shell's origin is ephemeral, so a device pointing at a remote api
typically needs `"*"` (an explicit, documented choice). WebSockets
are not subject to CORS. Example: the android app with the embedded
frontend and `api_base = "http://<pi>:3000"`, the Pi running an
full server with matching
`cors_origins`.

## Resource serving (`core::resources`, one router per host)

The router puts the leptos routes and the api in front; the resource
router — built once at bootstrap into the `Ctx` (`ctx.resources()`) by
the `resources` module — serves everything else and 404s the rest:

| host | resources |
| --- | --- |
| standalone (cli, watch) | `tower_http::ServeDir` on the resolved `site_root` |
| shell release | `ServeDir::with_backend` with the module's private `TauriBackend` — the fs plugin opens real files on desktop and APK assets (as fds) on Android; decoding, traversal guard, mime, `ETag` and ranges are ServeDir's, identical everywhere |
| shell dev | reverse proxy to the watch (`axum-reverse-proxy`) |

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
| `site_root` | see below | frontend bundle dir (`site` builds) |
| `api_base` | none (same origin) | origin the client sends api/ws to |
| `cors_origins` | empty (no CORS) | origins allowed on `/api` (`"*"` = any) |
| `log_to_file` | `false` | daily-rolling file in the app log dir |
| `dev.upstream` | `http://127.0.0.1:3001` | watch server the dev proxy targets |

The cli fails fast on an invalid file (systemd must see it); unknown
fields are rejected. `serve --host`/`--port` override `listen`.
`dev.upstream` must match `site-addr` in the leptos metadata and
`devUrl` in tauri.conf.json — three places, kept aligned by hand.

`site_root` convention: absolute paths as-is; relative paths resolve
against the config file's directory (cli) or `resource_dir` (shell);
absent = `target/site` (cli dev) / the bundled `site` map (shell).
The deb ships `/etc/tauri-leptos/config.toml` with the explicit
`/usr/share/tauri-leptos/site` path.

Every entrypoint bootstraps through `bootstrap::Ctx` (strict:
first run seeds the defaults, an invalid config refuses to start) and
runs through `core::app`:

```rust
let app = app(ctx)?;          // binds now, host-inferred (config listen
let addr = app.addr();        //   standalone, ephemeral in the shell)
app.start().await?;           // router + serve; or spawn it on a runtime
```

The factory receives the bound address (the site router needs it, and
an ephemeral port exists only after the bind); `Serving::addr()` is
what the shell puts in the window URL before spawning the future on
`tauri::async_runtime`.

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
cargo tauri dev              # spawns the watch; window + browser on the
                             # ephemeral origin (logged at startup)
cargo tauri build            # production bundle (embedded frontend)
cargo leptos build --release # site for plain-cargo servers
```

## Standalone / Raspberry Pi

`tauri-leptos-cli serve` (no flags) runs the single-origin server
standalone: site and bind from the config — the deb ships
`/etc/tauri-leptos/config.toml` (a conffile: apt keeps local edits)
with `listen = "0.0.0.0:3000"` and the site path, and the unit is a
bare `serve`. A remote api server is the same
full server with `cors_origins` set. Packaging:
`scripts/build-deb.sh` → deb with binary+site+system unit
(enable/start on install). See
[install/raspberry-pi.md](install/raspberry-pi.md).

## Out of scope for now

e2e testing; iOS (never started — would follow the desktop path,
bundle resources are real files there).
