# Raspberry Pi (headless server)

The headless binary has no Tauri/webkit dependencies. The release
artifact is a **pair**: the binary and the `site/` frontend directory —
they version together, ship them together.

## Build

On the Pi (or any aarch64 Linux host):

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos
cargo leptos build --release
cargo build --release -p tauri-leptos-cli --features frontend
```

Cross-compiling from another machine: cargo-leptos supports it natively —
set `bin-target-triple = "aarch64-unknown-linux-gnu"` in
`[[workspace.metadata.leptos]]` (add `bin-cargo-command = "cross"` if the
host cannot link aarch64); the binary lands under
`target/server/aarch64-unknown-linux-gnu/release/`, `target/site` as
usual.

## Install

```bash
sudo install -m 755 target/release/tauri-leptos-cli /usr/local/bin/
sudo mkdir -p /usr/local/share/tauri-leptos
sudo cp -r target/site /usr/local/share/tauri-leptos/site
```

`/usr/local/share/tauri-leptos/site` is the built-in default
`site_root` on Linux; a future install script owns these steps.

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
`/var/lib/tauri-leptos`, etc., and export the matching `*_DIRECTORY` env
vars that the app's path resolution picks up. `config.toml` (seeded on
first run under `/etc/tauri-leptos`) can replace the `--listen` flag.
An invalid config makes serve exit non-zero — visible in
`systemctl status`.

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now tauri-leptos
journalctl -u tauri-leptos -f        # logs (the app writes to stderr)
```

## Manual runs

```bash
tauri-leptos-cli serve --app-dir ~/tl-data --site-root target/site --log-to-file
```

(`TAURI_LEPTOS_APP_DIR` works too; an empty value acts as unset.)
