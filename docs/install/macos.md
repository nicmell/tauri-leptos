# macOS (desktop)

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos tauri-cli
```

## Build

```bash
cargo tauri build -f ssr
```

`-f ssr` compiles the in-process SSR server into the shell (without it
the app expects a running `cargo leptos watch` — that is the dev
configuration). `cargo leptos build --release` runs automatically first
and the frontend is bundled into the app's Resources
(`bundle.resources`), where the server reads it at runtime. Bundles land
in `target/release/bundle/` (`.app`, `.dmg`).

## Run

Launch the app — it binds the server on the configured port (default
`127.0.0.1:3000`, `config.toml` in
`~/Library/Application Support/com.nick.tauri-leptos`) and opens the
window on it. Logs go to stderr; `RUST_LOG` adjusts verbosity. Path
table in the [architecture doc](../architecture.md#paths).

For development use `mprocs` + `cargo tauri dev` (see
[architecture](../architecture.md#dev-workflows)).
