# Linux (systemd appliance)

The cli serves the whole app (SSR pages + API + WS) on one origin and
ships as a **deb** with its systemd unit. It links no Tauri/webkit —
**no GTK/WebKit packages needed** on the server. The reference target is
Debian 13 "trixie" aarch64 (a Raspberry Pi 5), but nothing here is
board-specific.

## Prerequisites (once)

```bash
# rust + tools, on whichever machine builds the deb
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos cargo-deb   # (or cargo-binstall them)
```

## Release flow

```bash
ssh <host>
cd tauri-leptos && git pull
./scripts/build-deb.sh
sudo apt install ./target/debian/tauri-leptos-cli_*.deb
```

Building on the target keeps the architecture right; cross-building works
too, the deb is just arch-specific.

The install **enables and starts** `tauri-leptos-cli.service`
automatically; upgrades (same commands) restart it. Removal stops and
disables it (`sudo apt remove tauri-leptos-cli`).

What the deb contains: `/usr/bin/tauri-leptos-cli`, the release site at
`/usr/share/tauri-leptos/site`, and the unit at
`/usr/lib/systemd/system/tauri-leptos-cli.service`
(`crates/app-cli/debian/` in the repo). The unit runs under
`DynamicUser=yes` — no account to create. Swap it for `User=`/`Group=`
if the service needs a fixed uid, group access to a device, or write
access to its own config.

## Verify

```bash
systemctl status tauri-leptos-cli.service
journalctl -u tauri-leptos-cli.service -f      # logs (stderr → journald)
curl http://<host>:3000/api/hello              # from the LAN
```

Browse `http://<host>:3000` — the whole app (page, API, WS) is served
there; the API is not authenticated, keep it on a trusted network.

Config: systemd's `ConfigurationDirectory=` owns `/etc/tauri-leptos`;
state under `/var/lib/tauri-leptos`.

## Manual runs (no deb)

```bash
cargo leptos build --release
cargo run -p tauri-leptos-cli -- serve --host 0.0.0.0
```

For a server feeding remote frontends, set `cors_origins` in the config
to the frontend origins (`"*"` for tauri shells — their origin is
ephemeral); the site the server also carries is harmless.

Site and bind come from the config. The deb installs
`/etc/tauri-leptos/config.toml` (a conffile — apt keeps your edits on
upgrade) with `listen = "0.0.0.0:3000"` and
`site_root = "/usr/share/tauri-leptos/site"`; relative `site_root`
paths resolve against the config's own directory. On a dev machine
the default is `target/site` (or use `--app-dir appdir`).
