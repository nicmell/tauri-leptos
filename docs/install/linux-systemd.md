# Linux (systemd appliance)

The cli serves the app (pages, server functions and site files) on one address. It ships as a deb with a systemd unit. It links no Tauri or WebKit code, so the server needs no GTK or WebKit packages. The reference target is Debian 13 "trixie" on aarch64 (a Raspberry Pi 5), but nothing here is specific to that board.

## Prerequisites

On the machine that builds the deb:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos cargo-deb   # or cargo binstall them
```

## Release flow

```bash
ssh <host>
cd tauri-leptos && git pull
./scripts/build-deb.sh
sudo apt install ./target/debian/tauri-leptos-cli_*.deb
```

A build on the target gets the architecture right. A cross build also works. The deb is for one architecture only.

The install enables and starts `tauri-leptos-cli.service`. An upgrade with the same commands restarts it. `sudo apt remove tauri-leptos-cli` stops and disables it.

The deb contains:

- `/usr/bin/tauri-leptos-cli`
- the release site, in `/usr/share/tauri-leptos/site`
- the unit, in `/usr/lib/systemd/system/tauri-leptos-cli.service` (from `crates/app-cli/debian/`)

The unit runs `tauri-leptos-cli --host 0.0.0.0 --port 3000` with `LEPTOS_SITE_ROOT=/usr/share/tauri-leptos/site`. It uses `DynamicUser=yes`, so you create no account.

## Change the address

The address comes only from the command line. To change it, override `ExecStart` with `sudo systemctl edit tauri-leptos-cli.service`:

```ini
[Service]
ExecStart=
ExecStart=/usr/bin/tauri-leptos-cli --host 0.0.0.0 --port 8080
```

## Verify

```bash
systemctl status tauri-leptos-cli.service
journalctl -u tauri-leptos-cli.service -f      # the output goes to journald
curl http://<host>:3000/                       # from the LAN
```

Open `http://<host>:3000` in a browser. The server functions have no authentication, so keep the server on a trusted network.

## Manual runs, without the deb

```bash
cargo leptos build --release --frontend-only
cargo run --release -p tauri-leptos-cli -- --host 0.0.0.0
```

From the repository root, the cli finds the site in `target/site`. From another directory, set `LEPTOS_SITE_ROOT`.
