# Raspberry Pi (headless server)

The headless binary (`tauri-leptos-cli`) has no Tauri/webkit
dependencies — it is a plain axum server with the frontend embedded.

## Build

On the Pi (or any aarch64 Linux host):

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk build --release
cargo build --release -p tauri-leptos-cli
```

Cross-compiling from another machine: build `trunk build --release`
first, then `cargo build --release -p tauri-leptos-cli --target
aarch64-unknown-linux-gnu` with your preferred cross toolchain (e.g.
`cross`). The frontend is embedded at compile time, so the single
binary is the whole deployment artifact.

## Install

```bash
sudo install -m 755 target/release/tauri-leptos-cli /usr/local/bin/
```

## systemd unit

`/etc/systemd/system/tauri-leptos.service`:

```ini
[Unit]
Description=tauri-leptos server
After=network.target

[Service]
ExecStart=/usr/local/bin/tauri-leptos-cli serve --listen 0.0.0.0:3000
DynamicUser=yes
ConfigurationDirectory=tauri-leptos
StateDirectory=tauri-leptos
CacheDirectory=tauri-leptos
LogsDirectory=tauri-leptos
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

The `*Directory=` directives make systemd own `/etc/tauri-leptos`,
`/var/lib/tauri-leptos`, etc., and export the matching
`*_DIRECTORY` env vars, which the app's path resolution picks up
automatically (see [architecture](../architecture.md#paths)).

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now tauri-leptos
journalctl -u tauri-leptos -f        # logs (the app writes to stderr)
```

## Manual runs

For an ad-hoc run with everything under one folder:

```bash
tauri-leptos-cli serve --app-dir ~/tauri-leptos-data --log-to-file
```

(or `TAURI_LEPTOS_APP_DIR=…`; an empty value acts as unset).
