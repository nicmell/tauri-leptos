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
  (feature `ssr`, the watch server run by `cargo leptos watch`, :3001).
- `crates/app-core` — config/paths/logging/api_router; no Leptos/Tauri.
- `crates/app-cli` — the standalone server (+ `config write|validate`).
- `src-tauri` — shell; non-dev builds embed the in-process server
  (`cfg(dev)` is its only branch).

## Commands

```bash
cargo tauri dev -- --no-default-features --features dev   # desktop dev
cargo leptos watch                              # frontend half (:3001)
cargo run -p tauri-leptos-cli --no-default-features --features dev -- serve  # api half, entry :3000
cargo tauri build                               # production bundle
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
- `serve` = the single-origin server (systemd/Pi); bind from
  `--host`/`--port` over the configured `listen`.
- Server functions stay stateless by convention; state lives behind
  `/api` and `/ws` in `core::server::api_router` (in-memory state
  resets on dev rebuilds and redeploys alike).
- Ports: 3000 = app origin (cli entry/serve default), 3001 = watch
  (internal; `dev.upstream` in config, leptos `site-addr`, `devUrl` must
  all match), 3002 = reload, ephemeral = shell. Build features pick the
  content: `site` (default) = embedded frontend, `dev` = api + reverse
  proxy to the watch, neither (cli) = api only.
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
