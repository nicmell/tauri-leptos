# Architecture

A pure-Rust application template: a Leptos SSR app with one axum router, served two ways. The cli serves it on an address you choose. The Tauri app serves it inside its window through [tauri-plugin-leptos-ssr](https://github.com/nicmell/tauri-plugin-leptos-ssr).

## Crate map

```
crates/ui        the Leptos app. Feature hydrate is the wasm client.
                 Feature ssr adds server::router(options, config): the
                 pages and server functions, without a fallback.
crates/app-cli   tauri-leptos-cli: the router plus the site files
                 (ServeDir) on --host/--port. cargo-leptos runs it as
                 the bin-package.
src-tauri        the Tauri app: tauri-plugin-log, the plugin with the
                 router, a window on the plugin's URL, and the demo
                 command greet.
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

The leptos `site-addr`, the cli defaults and the Tauri `devUrl` must agree. Two tests check them: one in `crates/app-cli/src/main.rs` and one in `src-tauri/tests/config.rs`.

## App config

`AppConfig` in `crates/ui/src/config.rs` is empty for now. The router provides it as Leptos context to server rendering and to server functions. Both entry points pass `AppConfig::default()`. A new field gets its value where an entry point builds the struct, for example from a cli flag.

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
