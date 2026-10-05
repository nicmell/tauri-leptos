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

The `beforeBuildCommand` runs `cargo leptos build --release --frontend-only` first. Tauri embeds the site in the app, and the plugin dispatches each request to the router in process. The bundles land in `target/release/bundle/` (`.app`, `.dmg`).

## Run

Open the app. It needs no files outside the bundle. It opens one network port, the websocket of `AppConfig.ws_addr` on `127.0.0.1:3002`. tauri-plugin-log writes the log to stdout and to `~/Library/Logs/com.example.tauri-leptos/`.

For development, run `cargo tauri dev`. See [the dev workflows](../architecture.md#dev-workflows).
