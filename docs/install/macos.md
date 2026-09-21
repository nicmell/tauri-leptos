# macOS (desktop)

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos tauri-cli
```

## Build

```bash
cargo tauri build
```

Non-dev builds compile the in-process single-origin server into the shell
(without it the app expects a running `cargo leptos watch` — that is
the dev configuration). `cargo leptos build --release` runs first
automatically and the site is bundled into the app's Resources.
Bundles land in `target/release/bundle/` (`.app`, `.dmg`).

## Run

Launch the app — it serves everything on an ephemeral local port and opens
the window there. `config.toml` lives in
`~/Library/Application Support/com.nick.tauri-leptos` (only the cli
bind address and file logging are configurable). Logs go to stderr;
`RUST_LOG` adjusts verbosity.

For development: `cargo tauri dev` (see
[architecture](../architecture.md#dev-workflows)).
