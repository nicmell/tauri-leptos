# Architecture

A pure-Rust application template: a Leptos SSR app with one axum router, served two ways. The cli serves it on an address you choose. The Tauri app serves it inside its window through [tauri-plugin-leptos-ssr](https://github.com/nicmell/tauri-plugin-leptos-ssr).

## Crate map

```
crates/ui        the Leptos app. Feature hydrate is the wasm client.
                 Feature ssr adds server::router(options, config): the
                 pages and server functions, without a fallback, and
                 socket::router, the websocket at /ws.
crates/protocol  the JSON messages between the page, its web worker and
                 the websocket server, and their encoding.
crates/worker    the web worker that holds the websocket.
crates/app-cli   tauri-leptos-cli: the router, the websocket and the
                 site files (ServeDir) on --host/--port. cargo-leptos
                 runs it as the bin-package.
src-tauri        the Tauri app: tauri-plugin-log, the plugin with the
                 router, the websocket on AppConfig.ws_addr, a window on
                 the plugin's URL, and the demo command greet.
```

## Two hosts, one router

`crates/ui/src/server.rs` builds the router. Both hosts call it and add their own fallback for the site files.

```
dev      cargo leptos watch   the cli on 127.0.0.1:3000, live reload on 3001
         cargo tauri dev      the window loads leptos://localhost/, and the
                              plugin forwards each request to the watch
release  the cli              the router and target/site on --host/--port
         the Tauri app        the plugin dispatches to the router in process,
                              with the site that Tauri embeds in the app
```

The plugin README describes the transport. The `leptos` scheme carries GET and HEAD requests. IPC carries requests with a body and event streams. On Android, the plugin's origin is `http://leptos.localhost`.

The Tauri dev build compiles the router but does not run it, because it forwards to the watch. So a change in `crates/ui` needs no new Tauri build, and `.taurignore` keeps `cargo tauri dev` from starting one.

## Ports

| Port | Use |
| --- | --- |
| 3000 | the cli: the watch server, and the default of `--host` and `--port` |
| 3001 | the cargo-leptos live-reload socket |
| 3002 | the Tauri app's websocket, `AppConfig.ws_addr` |

The leptos `site-addr`, the cli defaults and the Tauri `devUrl` must agree, and 3002 must differ from both other ports. Tests in `crates/app-cli/src/main.rs` and `src-tauri/tests/config.rs` check this.

## App config

`AppConfig` in `crates/ui/src/config.rs` holds `ws_addr`, the address of the Tauri app's websocket. The router provides it as Leptos context to server rendering and to server functions. Both entry points pass `AppConfig::default()`. A new field gets its value where an entry point builds the struct, for example from a cli flag.

## The websocket worker

```
page (wasm)  --postMessage, JSON-->  worker (wasm)  --ws://, JSON-->  /ws
```

- The worker runs a second instance of the wasm that cargo-leptos builds. `crates/ui/public/worker.js` imports the app's JS glue and calls `worker_main`. So the worker needs no build step of its own, and Tauri embeds `worker.js` with the site.
- The worker opens a plain browser websocket. Tauri IPC does not exist in a worker, so the socket cannot pass through it.
- In the Tauri app, the page connects to `ws://{ws_addr}/ws`. The Tauri app serves the websocket there on loopback. The page reads `ws_addr` from a `SharedValue`, which the server fills from the `AppConfig` context during server rendering.
- In a browser, the page connects to `/ws` on its own origin, which the cli serves.
- In Tauri dev, the watch renders the page, so `ws_addr` comes from the cli's `AppConfig`. Both entry points use `AppConfig::default()`, so the address matches the one that the Tauri app binds.
- `OriginPolicy` decides which pages may open the websocket:
  - The cli uses `SameOrigin`: the `Origin` header must name the `Host` that the request went to.
  - The Tauri app uses `Only`, with the plugin's origin from `LeptosSsr::origin()`: `leptos://localhost` on macOS, `http://leptos.localhost` on Android. A same-origin rule there would admit a DNS-rebinding page, because such a page sends an `Origin` that matches its `Host`.
- The port is fixed, so a second instance of the app cannot bind it. The worker of the second instance then reaches the first instance's server.
- The connection is not encrypted. Android release builds refuse cleartext even to loopback, so the app's network security config allows it to `127.0.0.1` (see [install/android.md](install/android.md)).

## Asset names and site builds

- `.cargo/config.toml` sets `LEPTOS_OUTPUT_NAME` at compile time. Leptos derives the asset URLs from it, for example `/pkg/tauri-leptos.wasm`. Only cargo-leptos builds set it on their own. Without it, plain cargo builds ask for the wasm-bindgen name `_bg.wasm`, and hydration fails. The Tauri app and the deb binary are plain cargo builds.
- Dev cargo-leptos builds compile Leptos with other flags. A page from such a build hydrates only against its own watch server. So a server built with plain cargo serves a release site, from `cargo leptos build --release --frontend-only`. Tauri runs that command in `beforeBuildCommand`.
- `bin-target-dir = "target/server"` gives the watch its own target directory. Without it, the watch and the Tauri dev build rebuild each other's dependencies.
- `hash-files` stays off, because the plugin looks for the site files under their plain names.

## Dev workflows

```bash
cargo tauri dev              # the watch, then the window
cargo leptos watch           # the watch alone, for a browser on http://127.0.0.1:3000
cargo leptos serve --release # the release cli, as a deployment runs it
cargo tauri android dev      # Android, see install/android.md
```

## Standalone server

The cli takes its address only from `--host` and `--port`. Leptos reads the site root from `LEPTOS_SITE_ROOT`, with the default `target/site`. The deb sets it in its systemd unit. See [install/linux-systemd.md](install/linux-systemd.md).

## Out of scope for now

End-to-end tests and iOS.
