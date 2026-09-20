# tauri-leptos

Pure-Rust app: Leptos SSR on axum wrapped by a Tauri 2 shell — one
origin in production, standard Tauri flow. Target of a progressive
migration from `sc-app2` (git submodule, React/TS — reference only,
not part of the build).

## Workspace layout

- `Cargo.toml` — virtual workspace (resolver 3): shared package fields,
  `workspace.dependencies`, `workspace.lints`, release profile, and the
  `[[workspace.metadata.leptos]]` block (bin-package = lib-package =
  tauri-leptos-ui). `.cargo/config.toml` pins `LEPTOS_OUTPUT_NAME`
  (asset names at compile time — plain cargo builds need it too).
- `crates/ui` — Leptos app: lib (feature `hydrate`, wasm client) + bin
  (feature `ssr`, the dev frontend-server run by `cargo leptos watch`).
- `crates/app-core` — config/paths/logging/api_router; no Leptos/Tauri.
- `crates/app-cli` — the API server (+ `config write|validate`).
- `src-tauri` — shell; feature `ssr` = in-process production server.

## Commands

```bash
cargo tauri dev                                 # desktop dev (spawns the watch)
mprocs                                          # browser dev: watch :3000 + api :3001
cargo tauri build -f ssr                        # production bundle
cargo leptos build                              # dev site + frontend-server
cargo leptos build --release                    # site for plain-cargo servers
./scripts/build-deb.sh                          # Pi deb (binary+site+unit)
cargo check --workspace
cargo clippy --workspace --all-targets
cargo clippy -p tauri-leptos-ui --features ssr
cargo clippy -p tauri-leptos-ui --features hydrate --target wasm32-unknown-unknown
cargo nextest run --workspace --no-tests=pass
cargo nextest run -p tauri-leptos-ui --features ssr
cargo fmt --all && leptosfmt crates/ui/src
cargo deny check
bacon                                           # watch loop (c=clippy w=wasm t=test d=doc s=serve)
```

## Conventions

- Edition 2024. Lints from `workspace.lints` — fix warnings, don't
  allow them without a stated reason.
- clippy pedantic on (warn); CI runs `-Dwarnings`.
- Formatting: rustfmt + leptosfmt; a Claude Code hook formats edited
  .rs files automatically.
- Dependencies: versions only in `workspace.dependencies`; members use
  `dep.workspace = true`. New dependencies must be ≥ 7 days old.
- `serve` = full single-origin server (systemd/Pi); `serve --headless`
  = api-only, the dev split half.
- Server functions are stateless (in dev they run in the frontend
  process); state lives behind `/api` and `/ws` in the API server.
- The frontend origin (`127.0.0.1:3000`) is fixed by design; only the
  API address is configurable (`config.toml`, `api_addr`).
- Any server built with plain cargo serves a **release** site — dev
  cargo-leptos builds only hydrate against their own watch.
- No LICENSE yet: crates are `publish = false`, cargo-deny ignores
  them as private.
- One feature per commit; every commit leaves the workspace green
  (`cargo check` + `cargo leptos build`).

## CI

`.github/workflows/ci.yml`: checks (fmt/leptosfmt, clippy native +
ui-ssr + ui-hydrate-wasm32, nextest incl. ssr tests, cargo leptos
build) and deny. The checks job installs Tauri's webkit apt deps —
keep the list in sync with Tauri's Linux prerequisites when bumping
Tauri.

## Out of scope right now

e2e; iOS (never started).
