# tauri-leptos

A pure-Rust application template: **Leptos SSR** rendered by an **axum**
server, wrapped by a **Tauri 2** shell. Single origin in production —
page, API, and WebSocket from one in-process server; standard Tauri
dev/build flow; server functions included.

Full picture: [docs/architecture.md](docs/architecture.md).

```
crates/ui        Leptos app: wasm client (hydrate) + SSR pages and server fns (ssr)
crates/app-core  config + paths + logging + API router (/api, /ws) + the one router
crates/app-cli   tauri-leptos-cli: the standalone server + config subcommands
src-tauri        Tauri shell (non-dev builds embed the in-process server)
appdir/          repo-local app root for reproducible dev runs
docs/            architecture, per-platform install, template updates
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
cargo tauri dev          # spawns the watch, window on an ephemeral origin
```

The dev shell serves one origin (api in-process + pages proxied from
the watch): api state survives frontend rebuilds (try `curl -X POST .../api/counter`),
and the origin is a plain http server — open the URL logged at startup
in a browser for browser work. Everything is same-origin and
relative, in dev and production alike.

## Make it yours

Straight into a new project, prompting for the display name, identifier
and author:

```bash
cargo generate --git https://github.com/nicmell/tauri-leptos \
    --name acme-app --allow-commands
```

`--allow-commands` lets the template's hook run the rename script (without
it, cargo-generate asks before running it). In a clone, run the same
script yourself:

```bash
./scripts/rename-app.sh --name Acme --slug acme-app \
    --identifier com.acme.app --author "You <you@acme.com>" --remove-self
```

Either way it renames crates, the bundle identifier, the Android package, the
deb/systemd paths and the docs in one pass (`--dry-run` shows the plan
first; `--remove-self` drops the script once your app no longer needs
it). Then drop the demo and write your own:

| Delete | Then |
| --- | --- |
| `crates/app-core/src/server/demo.rs` | start `api_router` from `axum::Router::new()` |
| `crates/ui/src/demo.rs` | point the route in `crates/ui/src/app.rs` at your page |
| `crates/ui/assets/public/*.svg` | your own assets (`styles.css` stays) |
| the demo assertions in `crates/app-core/tests/` | tests for your routes |

Also `cargo tauri icon <your.png>` for the icon set, and replace
`LICENSE` with your app's.

## Where things go

| To add | Edit |
| --- | --- |
| a page or route | `crates/ui/src/app.rs` (+ a module per page) |
| a server function (typed RPC, served under `/fn`) | any `#[server]` fn in `crates/ui` |
| a stateful endpoint or WebSocket (`/api`, `/ws`) | `api_router` in `crates/app-core/src/server.rs` |
| a config field | `AppConfig` in `crates/app-core/src/config.rs` |
| an app directory | `crates/app-core/src/paths.rs` |
| a Tauri command or plugin | `src-tauri/src/lib.rs` |
| a cli subcommand | `crates/app-cli/src/main.rs` |
| styles or static files | `crates/ui/assets/` |

Two rules the layout depends on: `/fn` is frontend-local RPC and `/api`
is the shareable stateful surface (they must not shadow each other), and
app-core owns the single router — the entrypoints have no compile-time
branches. [docs/architecture.md](docs/architecture.md) has the why;
[docs/template-updates.md](docs/template-updates.md) covers pulling later
template changes into your app.

## Production build

```bash
cargo tauri build                 # desktop bundle
./scripts/build-deb.sh            # Linux deb (binary + site + systemd unit)
cargo tauri android build         # Android APK (server in-process, assets from the APK)
```

`cargo leptos build --release` runs first automatically; the site is
bundled into the app's resources and served by the in-process server.

## Tests and quality

```bash
cargo nextest run --workspace --no-tests=pass          # native tests
cargo nextest run -p tauri-leptos-core             # incl. SSR render tests
cargo clippy --workspace --all-targets
cargo clippy -p tauri-leptos-ui --features ssr
cargo clippy -p tauri-leptos-ui --features hydrate --target wasm32-unknown-unknown
cargo fmt --all && leptosfmt crates/ui/src
cargo deny check
bacon                                                  # watch loop (c/w/t/d/s)
```

CI runs all of the above on every PR. Conventions live in
[CLAUDE.md](CLAUDE.md); RustRover run configurations in `.run/`.

## License

MIT-0 ([LICENSE](LICENSE)) — no attribution required, so an app
generated from this template just replaces the file with its own.
