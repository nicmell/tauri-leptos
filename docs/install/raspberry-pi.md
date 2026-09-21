# Raspberry Pi (systemd appliance)

The cli serves the whole app (SSR pages + API + WS) on one origin and
ships as a **deb** with its systemd unit. It links no Tauri/webkit —
**no GTK/WebKit packages needed** on the Pi.

The audio stack (jackd → scsynth → StrudelDirt as system units, the
`RemoveIPC`/linger gotcha, quarks) is documented in [`rpi/`](../../rpi/)
— the app is the fourth unit of that stack and follows its pattern.

## Prerequisites (once)

```bash
# rust + tools
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos cargo-deb   # (or cargo-binstall them)
```

## Release flow

```bash
ssh <pi>
cd tauri-leptos && git pull
./scripts/build-deb.sh
sudo apt install ./target/debian/tauri-leptos-cli_*.deb
```

The install **enables and starts** `tauri-leptos-cli.service`
automatically; upgrades (same commands) restart it. Removal stops and
disables it (`sudo apt remove tauri-leptos-cli`).

What the deb contains: `/usr/bin/tauri-leptos-cli`, the release site at
`/usr/share/tauri-leptos/site`, and the unit at
`/usr/lib/systemd/system/tauri-leptos-cli.service`
(`crates/app-cli/debian/` in the repo — **edit `User=`/`Group=` to
match your machine** before building; it defaults to the reference
Pi's `nick`/`audio`, the same user as scsynth so the future SHM scope
access shares ownership).

## Verify

```bash
systemctl status tauri-leptos-cli.service
journalctl -u tauri-leptos-cli.service -f      # logs (stderr → journald)
curl http://<pi-ip>:3000/api/hello             # from the LAN
```

Browse `http://<pi-ip>:3000` — the whole app (page, API, WS) is served
there; the API is not authenticated, keep it on a trusted network.

Config: systemd's `ConfigurationDirectory=` owns `/etc/tauri-leptos`
(seeded `config.toml` on first run); state under
`/var/lib/tauri-leptos`. Remember the linger requirement from
[`rpi/raspberry-pi.md`](../../rpi/raspberry-pi.md) — it protects
scsynth's SHM, which this app will map.

## Manual runs (no deb)

```bash
cargo leptos build --release
cargo run -p tauri-leptos-cli -- serve --host 0.0.0.0 --port 3000
```

For an api-only Pi (frontends elsewhere) build with
`--no-default-features` and set `cors_origins` in the config to the
frontend origins (`"*"` for tauri shells — their origin is ephemeral).

The site comes from `site_root` in the config; the Linux default is
`/usr/share/tauri-leptos/site` (where the deb installs it). On a dev
machine set `site_root = "target/site"` in the config (or use
`--app-dir appdir`).
