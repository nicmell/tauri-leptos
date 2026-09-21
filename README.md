# tauri-leptos

A pure-Rust application template: **Leptos SSR** rendered by an **axum**
server, wrapped by a **Tauri 2** shell. Single origin in production —
page, API, and WebSocket from one in-process server; standard Tauri
dev/build flow; server functions included.

Full picture: [docs/architecture.md](docs/architecture.md).

```
crates/ui        Leptos app: wasm client (hydrate) + dev frontend-server (ssr bin)
crates/app-core  config + paths + logging + API router (/api, /ws)
crates/app-cli   tauri-leptos-cli: the standalone server + config subcommands
src-tauri        Tauri shell (feature ssr = in-process production server)
appdir/          repo-local app root for reproducible dev runs
```

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos tauri-cli
# optional dev tools
cargo install bacon cargo-nextest cargo-deny leptosfmt
```

## Quick start

```bash
cargo tauri dev          # desktop: spawns cargo leptos watch, window on :3000
```

For browser work:

```bash
cargo leptos watch       # everything on :3000 (SSR + api + ws, hot reload)
```

`view!`/CSS edits hot-patch in place; edits to Rust logic restart the
dev server (in-memory demo state resets, like a redeploy). Everything
is same-origin and relative, in dev and production alike.

## Production build

```bash
cargo tauri build -f ssr          # desktop bundle
./scripts/build-deb.sh            # Raspberry Pi deb (binary + site + systemd unit)
cargo tauri android build         # Android APK (server in-process, assets from the APK)
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
