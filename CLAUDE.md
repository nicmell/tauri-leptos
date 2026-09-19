# tauri-leptos

Pure-Rust desktop/web app: Tauri 2 backend + Leptos 0.8 (CSR) frontend,
built with Trunk. Target of a progressive migration from `sc-app2`
(git submodule, React/TS — reference only, not part of the build).

## Workspace layout

- `Cargo.toml` — virtual workspace (resolver 3): shared package fields,
  dependency versions (`workspace.dependencies`), lint policy
  (`workspace.lints`), release profile. New crates go under `crates/`.
- `crates/ui` — Leptos frontend. `lib.rs` exposes components; `main.rs`
  is the wasm entry (tracing init + mount).
- `src-tauri` — Tauri shell. `tauri.conf.json` drives trunk via
  beforeDevCommand/beforeBuildCommand; frontend assets land in `dist/`.

## Commands

```bash
trunk serve                # frontend only, port 1420
cargo tauri dev            # full native app
trunk build                # bundle frontend to dist/
cargo check --workspace
cargo clippy --workspace --all-targets          # native
cargo clippy -p tauri-leptos-ui --target wasm32-unknown-unknown
cargo nextest run --workspace --no-tests=pass   # native tests
wasm-pack test --headless --chrome crates/ui    # wasm/browser tests
cargo fmt --all && leptosfmt crates/ui/src      # format
cargo deny check           # advisories/licenses/bans/sources
bacon                      # watch loop (c=clippy w=wasm t=test d=doc)
```

## Conventions

- Edition 2024. Lints come from `workspace.lints` — never add per-crate
  lint attributes without a reason; fix warnings, don't allow them.
- clippy pedantic is on (warn): CI runs with `-Dwarnings`, so warnings
  are build failures there.
- Formatting: rustfmt + leptosfmt (config at repo root); a Claude Code
  hook formats edited .rs files automatically.
- Dependencies: add versions in `workspace.dependencies` only; members
  reference with `dep.workspace = true`. Any new dependency must be at
  least 7 days old.
- No LICENSE yet: crates are `publish = false` and cargo-deny ignores
  them as private. Choosing a license lifts both.
- One feature per commit; every commit leaves the workspace green
  (`cargo check` + `trunk build`).

## CI

`.github/workflows/ci.yml`: checks job (fmt + leptosfmt check, clippy
native + wasm32 with `-Dwarnings`, nextest) and deny job. Tauri needs
webkit system packages on ubuntu — keep that apt list in sync with
Tauri's Linux prerequisites when bumping Tauri.
