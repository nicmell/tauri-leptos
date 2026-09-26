# tauri-leptos

Template for pure-Rust apps: Leptos SSR on axum wrapped by a Tauri 2
shell — one origin in production, standard Tauri flow. A generated app
renames the template first (`scripts/rename-app.sh`), then replaces the
demo endpoints and page.

## Workspace layout

- `Cargo.toml` — virtual workspace (resolver 3): shared package fields,
  `workspace.dependencies`, `workspace.lints`, release profile, and the
  `[[workspace.metadata.leptos]]` block (bin-package = tauri-leptos-cli,
  lib-package = tauri-leptos-ui). `.cargo/config.toml` pins
  `LEPTOS_OUTPUT_NAME`
  (asset names at compile time — plain cargo builds need it too).
- `crates/ui` — Leptos frontend, pure library: lib (feature
  `hydrate`, wasm client) + server module (feature `ssr`). The watch
  server IS the cli (`bin-package`), on :3001 via the committed
  `appdir/config/config.toml`.
- `crates/app-core` — everything server-side: config/paths/logging,
  api_router, resource routers, `Ctx::router` (depends on ui; feature
  `tauri` gates the shell-only bits).
- Demo code is confined to two files: `crates/app-core/src/server/demo.rs`
  (routes) and `crates/ui/src/demo.rs` (page + server fn). Deleting them
  is the documented way to start an app (README, "Make it yours"); keep
  new app code out of them.
- `crates/app-cli` — the standalone server (+ `config write|validate`).
- `src-tauri` — shell; non-dev builds embed the in-process server
  (`cfg(dev)` is its only branch).

## Commands

```bash
cargo tauri dev                                 # dev (watch spawned; browser = logged ephemeral URL)
cargo tauri build                               # production bundle
cargo leptos build                              # dev site + watch (cli) binary
cargo leptos build --release                    # site for plain-cargo servers
./scripts/build-deb.sh                          # Linux deb (binary+site+unit)
cargo check --workspace
cargo clippy --workspace --all-targets
cargo clippy -p tauri-leptos-ui --features ssr
cargo clippy -p tauri-leptos-ui --features hydrate --target wasm32-unknown-unknown
cargo clippy -p tauri-leptos --features tauri/custom-protocol  # shell release branch
cargo nextest run --workspace --no-tests=pass
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
- `serve` = the single-origin server (systemd deb); bind from
  `--host`/`--port` over the configured `listen`.
- Server functions stay stateless by convention and live under `/fn`
  (`server-fn-prefix` + the `SERVER_FN_PREFIX` env pin must match);
  state lives behind `/api` and `/ws` in `core::server::api_router`
  (in-memory state resets on dev rebuilds and redeploys alike).
- Ports: 3000 = app origin (cli entry/serve default), 3001 = watch
  (internal; `dev.upstream` in config, leptos `site-addr`, `devUrl` must
  all match), 3002 = reload, ephemeral = shell. Build features pick the
  content: the shell under `cfg(dev)` = api + reverse proxy to the
  watch; everything else = embedded frontend. No app cargo features,
  no compile branches in the entrypoints — core owns the ONE router
  (`Ctx::router`, host-inferred; `cfg!(dev)` flows in as data); ui is
  the leptos-only `server::router(addr, api_base)` (no core
  dependency, 404 on unknown paths).
- Any server built with plain cargo serves a **release** site — dev
  cargo-leptos builds only hydrate against their own watch.
- Template licensed MIT-0 (`license` in `workspace.package`): a
  generated app replaces LICENSE with its own. Crates stay
  `publish = false`, cargo-deny ignores them as private.
- Template plumbing: `scripts/rename-app.sh` (the rename),
  `scripts/generate.rhai` + `cargo-generate.toml` (the cargo-generate
  hook, which only forwards the prompted values to that script).
  `--remove-self` deletes all three. No liquid placeholders anywhere:
  they would stop the template from being a buildable app, which is the
  point of it. The hook does every substitution instead, so
  `cargo-generate.toml` switches liquid templating off for all files
  (`exclude`) — keep it that way.
  CI runs both paths: a generate smoke and a rename smoke.
- One feature per commit; every commit leaves the workspace green
  (`cargo check` + `cargo leptos build`).

## CI

`.github/workflows/ci.yml`: checks (fmt/leptosfmt, clippy native +
ui-ssr + ui-hydrate-wasm32, nextest incl. ssr tests, cargo leptos
build) and deny. The last checks step is the **rename smoke**: it runs
`scripts/rename-app.sh` and rebuilds — a new file carrying the template
names, or a name the script does not know, fails there. The checks job
installs Tauri's webkit apt deps — keep the list in sync with Tauri's
Linux prerequisites when bumping Tauri.

## Out of scope right now

e2e; iOS (never started).
