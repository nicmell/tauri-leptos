# tauri-leptos

A pure-Rust application template: **Tauri 2** shell + **Leptos** (CSR,
wasm) frontend + one shared **axum** server. The same app runs three
ways — desktop, Android, and headless server — all serving the UI, API,
and WebSocket from a single origin. No Tauri IPC, no CORS.

Full picture: [docs/architecture.md](docs/architecture.md).

```
crates/ui        Leptos frontend (Trunk → dist/)
crates/app-core  paths + logging + axum server (no Tauri)
crates/app-cli   tauri-leptos-cli headless binary (no Tauri)
src-tauri        Tauri shell (desktop + Android)
docs/            architecture + install guides
appdir/          repo-local app root for reproducible dev runs
```

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
# optional dev tools
cargo install bacon cargo-nextest cargo-deny leptosfmt
```

## Quick start

```bash
cargo tauri dev                                    # desktop app
cargo run -p tauri-leptos-cli -- serve             # headless, http://127.0.0.1:3000
```

`trunk build` runs automatically before tauri commands; for the
headless binary run it yourself first (debug builds then read `dist/`
from disk, release builds embed it).

## Local development

**UI iteration (browser, hot reload)** — two terminals:

```bash
cargo run -p tauri-leptos-cli -- serve --app-dir appdir   # backend on :3000
trunk serve                                               # UI on :1420, /api + /ws proxied
```

**Desktop**: `cargo tauri dev`. After a UI edit: `trunk build` and
reload the window (no Rust rebuild needed in debug).

**Reproducible runs**: `--app-dir appdir` (or
`TAURI_LEPTOS_APP_DIR=$PWD/appdir`) pins config/data/cache/logs under
the repo-local `appdir/` (gitignored). Without it, platform defaults
apply — see the [path table](docs/architecture.md#paths). The tauri
shell uses Tauri's own path resolver and ignores this override.

**Android**: see [docs/install/android.md](docs/install/android.md).

RustRover users: ready-made run configurations live in `.run/`
(including a compound `Dev: UI + Server`).

## Tests and quality

```bash
cargo nextest run --workspace --no-tests=pass   # native tests
wasm-pack test --headless --chrome crates/ui    # wasm/browser tests
cargo clippy --workspace --all-targets          # lints (pedantic, -Dwarnings in CI)
cargo clippy -p tauri-leptos-ui --target wasm32-unknown-unknown
cargo fmt --all && leptosfmt crates/ui/src      # format
cargo deny check                                # advisories/licenses/bans
bacon                                           # watch loop (c/w/t/d jobs)
```

CI runs all of the above on every PR. Conventions (workspace lints,
dependency policy, commit discipline) live in [CLAUDE.md](CLAUDE.md).

## Install / deploy

- [macOS desktop](docs/install/macos.md)
- [Android](docs/install/android.md)
- [Raspberry Pi headless + systemd](docs/install/raspberry-pi.md)
