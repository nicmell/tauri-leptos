# tauri-leptos

A pure-Rust application template: **Leptos SSR** rendered by an **axum**
server, wrapped by a **Tauri 2** shell. Single origin in production —
page, API, and WebSocket from one in-process server; standard Tauri
dev/build flow; server functions included.

Full picture: [docs/architecture.md](docs/architecture.md).

```
crates/ui        Leptos app: wasm client (hydrate) + dev frontend-server (ssr bin)
crates/app-core  config + paths + logging + API router (/api, /ws)
crates/app-cli   tauri-leptos-cli: the API server + config subcommands
src-tauri        Tauri shell (feature ssr = in-process production server)
appdir/          repo-local app root for reproducible dev runs
```

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos tauri-cli
# optional dev tools
cargo install bacon cargo-nextest cargo-deny leptosfmt mprocs
```

## Quick start

```bash
cargo tauri dev          # desktop: spawns cargo leptos watch, window on :3000
```

For browser work run the pair yourself:

```bash
mprocs                   # watch (:3000, hot reload) + API server (:3001)
```

UI edits hot-reload through the watch; the API server keeps its state
(try `/api/counter`). The page learns the API address from the
`api-base` meta the dev server injects; in production everything is
same-origin and relative.

## Production build

```bash
cargo tauri build -f ssr
```

`cargo leptos build --release` runs first automatically; the site is
bundled into the app's resources and served by the in-process server.

## Tests and quality

```bash
cargo nextest run --workspace --no-tests=pass          # native tests
cargo nextest run -p tauri-leptos-ui --features ssr    # SSR render tests
cargo clippy --workspace --all-targets
cargo clippy -p tauri-leptos-ui --features ssr
cargo clippy -p tauri-leptos-ui --features hydrate --target wasm32-unknown-unknown
cargo fmt --all && leptosfmt crates/ui/src
cargo deny check
bacon                                                  # watch loop (c/w/t/d/s)
```

CI runs all of the above on every PR. Conventions live in
[CLAUDE.md](CLAUDE.md); RustRover run configurations in `.run/`.
