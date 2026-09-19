# tauri-leptos

A pure-Rust application template: **Leptos SSR** rendered by an **axum**
server, wrapped by **Tauri 2** shells (desktop, Android) or run headless.
Single origin everywhere — page, API, and WebSocket from one server; no
CORS, no Tauri IPC. Server functions included.

Full picture: [docs/architecture.md](docs/architecture.md).

```
crates/ui          Leptos app (hydrate/ssr) + server functions
crates/app-core    config + paths + logging + API router (no Leptos/Tauri)
crates/app-cli     tauri-leptos-cli: dev API server / full server (--features frontend)
crates/dev-server  dev only: SSR + /api,/ws proxy (cargo-leptos bin)
src-tauri          Tauri shell (desktop + Android)
appdir/            repo-local app root for reproducible dev runs
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
mprocs                  # dev loop: SSR watch (:3000) + API server (:3001)
cargo tauri dev         # + desktop shell attached to :3000
```

Browse `http://127.0.0.1:3000`. UI edits hot-reload through
`cargo leptos watch`; the API server (and its state — try
`/api/counter`) is never restarted by UI work.

## Production builds

```bash
# headless (binary + target/site directory pair)
cargo leptos build --release
cargo build --release -p tauri-leptos-cli --features frontend

# desktop (in-process SSR, frontend bundled in Resources)
cargo tauri build -f ssr

# android (frontend shipped as site.tar, unpacked on first launch)
cargo tauri android build
```

Install guides: [macOS](docs/install/macos.md),
[Android](docs/install/android.md),
[Raspberry Pi + systemd](docs/install/raspberry-pi.md).

## Tests and quality

```bash
cargo nextest run --workspace --no-tests=pass          # native tests
cargo nextest run -p tauri-leptos-ui --features ssr    # SSR render tests
node scripts/e2e-smoke.mjs                             # hydration smoke (headless chrome)
cargo clippy --workspace --all-targets                 # + ui: --features ssr / hydrate(wasm32)
cargo fmt --all && leptosfmt crates/ui/src             # format
cargo deny check                                       # advisories/licenses/bans
bacon                                                  # watch loop (c/w/t/d/s jobs)
```

CI runs all of the above on every PR. Conventions live in
[CLAUDE.md](CLAUDE.md); RustRover run configurations in `.run/`.
