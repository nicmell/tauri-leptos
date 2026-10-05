# tauri-leptos

Template for pure-Rust apps: one Leptos SSR router with two hosts. A generated app renames the template first (`scripts/rename-app.sh`), then replaces the demo. A cli serves the router, and so does a Tauri app through [tauri-plugin-leptos-ssr](https://github.com/nicmell/tauri-plugin-leptos-ssr).

## Workspace layout

- `Cargo.toml`: the virtual workspace (resolver 3). It holds the shared package fields, `workspace.dependencies`, `workspace.lints`, the release profile and the `[[workspace.metadata.leptos]]` block (bin-package `tauri-leptos-cli`, lib-package `tauri-leptos-ui`). `.cargo/config.toml` sets `LEPTOS_OUTPUT_NAME`, which plain cargo builds need too.
- `crates/ui`: the Leptos app. Feature `hydrate` is the wasm client, and only cargo-leptos enables it. Feature `ssr` adds `server::router(options, config)`, without a fallback, because each host adds its own.
- `crates/ui/src/config.rs`: `AppConfig`, empty for now. The router provides it as context.
- `crates/protocol`: the demo's messages (`ClientMessage`, `ServerMessage`), their JSON encoding, and (feature `ssr`) the session that answers them. Its tests pin the JSON and run the session.
- `crates/core`: the pipe from a page to a session, transport only: frames of text or bytes (`Frame`, `PipeEvent`). Feature `hydrate` holds the page's end (`page::Pipe`, which picks the transport at run time), the bindings to `window.__TAURI__` (`tauri_ipc`) and the web worker (`worker_main`). Feature `ssr` holds `Session`, the route `/ws` and `pipes::Pipes` for the Tauri app. Both take the session that the app brings. The worker starts from `crates/ui/public/pipe-worker.js`. "The pipe" in `docs/architecture.md` describes both transports.
- `crates/app-cli`: the standalone server, configured by `--host` and `--port` only. `cargo leptos watch` runs it as the bin-package.
- `src-tauri`: the Tauri app. It registers the plugin with the ui router and opens its window on `webview_url("/")`. The commands `pipe_open`, `pipe_post` and `pipe_close` answer the pipe. When a page starts to load or a window closes, the app closes the pipes of that webview. `tests/config.rs` checks the plugin's requirements against the leptos metadata, and `withGlobalTauri`.
- The demo: `crates/ui/src/demo.rs`, the `greet` command in `src-tauri/src/lib.rs` with its binding in `crates/ui/src/tauri_ipc.rs`, and `crates/ui/public/*.svg`. Deleting them is the documented start of an app (README, "Make it yours"). Keep new app code out of them.
- The pipe's demo: `crates/protocol`, with its message variants and the tick and echoes of its session. An app replaces them, and the pipe stays.

## Commands

```bash
cargo tauri dev                                 # watch + window (run cargo leptos build once first)
cargo leptos watch                              # the watch alone, http://127.0.0.1:3000
cargo tauri build                               # production bundle
cargo leptos build --release --frontend-only    # release site for plain-cargo servers
./scripts/build-deb.sh                          # Linux deb (binary + site + unit)
cargo check --workspace
cargo clippy --workspace --all-targets
cargo clippy -p tauri-leptos-ui --features ssr
cargo clippy -p tauri-leptos-ui --features hydrate --target wasm32-unknown-unknown
cargo nextest run --workspace --no-tests=pass
cargo fmt --all && leptosfmt crates/ui/src
cargo deny check
bacon                                           # watch loop (c=clippy w=wasm t=test d=doc)
```

## Conventions

- Edition 2024. The lints come from `workspace.lints`. Fix warnings, and allow one only with a stated reason.
- clippy pedantic is on (warn), and CI runs `-Dwarnings`.
- Formatting: rustfmt and leptosfmt. A Claude Code hook formats edited `.rs` files.
- Dependencies: versions live only in `workspace.dependencies`, and members use `dep.workspace = true`. New dependencies must be at least 7 days old. Create the lockfile with `RUSTC_BOOTSTRAP=1 cargo generate-lockfile -Zunstable-options --publish-time <today minus 7 days>T00:00:00Z`, because `cargo update` has no such flag.
- tauri-plugin-leptos-ssr comes from git at a tag. `deny.toml` allows that one git source.
- Ports: 3000 for the cli (the watch, the `--port` default, `devUrl` and the leptos `site-addr`), 3001 for live reload. Tests check that they agree.
- Server functions use the Leptos default prefix `/api`.
- Any server built with plain cargo serves a release site, because dev cargo-leptos builds hydrate only against their own watch.
- The template is MIT-0 (`license` in `workspace.package`). A generated app replaces LICENSE with its own. Crates stay `publish = false`, and cargo-deny skips them as private.
- Template plumbing: `scripts/rename-app.sh` does the rename. `scripts/generate.rhai` and `cargo-generate.toml` form the cargo-generate hook, which only forwards the prompted values to that script. `--remove-self` deletes all three.
- No liquid placeholders anywhere. A placeholder breaks the build of the template itself, and a buildable template is the point. The hook does every substitution instead, so `cargo-generate.toml` switches liquid off for all files (`exclude`). Keep it that way.
- One feature per commit. Every commit leaves the workspace green (`cargo check` and `cargo leptos build`).

## CI

`.github/workflows/ci.yml` runs the checks and cargo-deny. The checks are fmt, leptosfmt, clippy (native, ui ssr, and ui hydrate on wasm32), nextest and `cargo leptos build`. Then come two smokes. The generate smoke runs cargo-generate on an archive of HEAD. The rename smoke runs `scripts/rename-app.sh` and rebuilds, so a new file with the template names, or a name the script does not know, fails there. The checks job installs Tauri's webkit apt packages. When you bump Tauri, keep that list in sync with Tauri's Linux prerequisites.

## Out of scope right now

End-to-end tests and iOS.
