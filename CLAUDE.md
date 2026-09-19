# tauri-leptos

Pure-Rust app: Leptos SSR on axum, Tauri 2 shells (desktop/Android) or
headless serve — one server, single origin, no IPC/CORS. Target of a
progressive migration from `sc-app2` (git submodule, React/TS —
reference only, not part of the build).

## Workspace layout

- `Cargo.toml` — virtual workspace (resolver 3): shared package fields,
  `workspace.dependencies`, `workspace.lints`, release profile, and the
  `[[workspace.metadata.leptos]]` block (bin-package = dev-server).
  `.cargo/config.toml` pins `LEPTOS_OUTPUT_NAME` (asset names at compile
  time — plain cargo builds need it too).
- `crates/ui` — Leptos app, features `hydrate` (wasm) / `ssr` (router).
- `crates/app-core` — config/paths/logging/api_router; no Leptos, no Tauri.
- `crates/app-cli` — headless binary; `--features frontend` = full server.
- `crates/dev-server` — dev SSR + /api,/ws proxy (run by cargo leptos watch).
- `src-tauri` — shell; feature `ssr` = in-process server (android: always).

## Commands

```bash
mprocs                                          # dev: watch :3000 + api :3001
cargo tauri dev                                 # desktop shell (attaches to :3000)
cargo leptos build                              # frontend + dev-server
cargo tauri build -f ssr                        # desktop release
cargo tauri android build --debug --target aarch64
cargo check --workspace
cargo clippy --workspace --all-targets          # native
cargo clippy -p tauri-leptos-ui --features ssr
cargo clippy -p tauri-leptos-ui --features hydrate --target wasm32-unknown-unknown
cargo nextest run --workspace --no-tests=pass
cargo nextest run -p tauri-leptos-ui --features ssr
node scripts/e2e-smoke.mjs                      # hydration smoke (needs frontend build)
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
- Server functions are stateless (in dev they run in the watch process);
  state lives behind `/api` and `/ws` in the API server.
- No LICENSE yet: crates are `publish = false`, cargo-deny ignores them
  as private.
- One feature per commit; every commit leaves the workspace green
  (`cargo check` + `cargo leptos build`).

## CI

`.github/workflows/ci.yml`: checks (fmt/leptosfmt, clippy native +
ui-ssr + ui-hydrate-wasm32, nextest incl. ssr tests, cargo leptos
build), e2e-smoke (headless chrome hydration), deny. The checks job
installs Tauri's webkit apt deps — keep the list in sync with Tauri's
Linux prerequisites when bumping Tauri.

## Android notes

`src-tauri/gen/android/` is committed (carries the
network-security-config for cleartext-to-loopback and its manifest
edit; re-running `android init` may clobber them — review diffs). The
frontend ships as `site.tar` (APK assets aren't enumerable files) and
is unpacked to app data on first launch per app version.
