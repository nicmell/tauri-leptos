# macOS (desktop)

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
```

## Build

```bash
cargo tauri build
```

`trunk build` runs automatically first (`beforeBuildCommand`). Bundles
land in `target/release/bundle/` (`.app`, `.dmg`).

## Run

Launch the app, or during development:

```bash
cargo tauri dev
```

The app binds its HTTP server on an ephemeral loopback port and opens
the window on it. Runtime paths follow the [architecture
doc](../architecture.md#paths): config/data under
`~/Library/Application Support/com.nick.tauri-leptos`, logs (when
enabled) under `~/Library/Logs/com.nick.tauri-leptos`. Logs go to
stderr; set `RUST_LOG` for verbosity.
