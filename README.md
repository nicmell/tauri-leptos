# tauri-leptos

A pure-Rust application template: one Leptos SSR app, served two ways. The cli serves it on an address you choose. The Tauri app serves it inside its window through [tauri-plugin-leptos-ssr](https://github.com/nicmell/tauri-plugin-leptos-ssr). In both hosts, a pipe connects the page to a Rust session: a web worker with a websocket in a browser, Tauri channels in the app.

Full picture: [docs/architecture.md](docs/architecture.md).

```
crates/ui        Leptos app: wasm client (feature hydrate), SSR router and server functions (feature ssr)
crates/core      the pipe from the page to a session: a web worker and a websocket, or Tauri channels
crates/protocol  the pipe's messages and their JSON encoding
crates/app-cli   tauri-leptos-cli: the router plus the site files on --host/--port
src-tauri        Tauri app: the router behind the plugin's leptos scheme, the pipe commands
docs/            architecture, per-platform install, template updates
```

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos tauri-cli
# optional dev tools
cargo install bacon cargo-nextest cargo-deny leptosfmt
```

## Quick start

```bash
cargo leptos build    # once: tauri-cli waits only 180 s for the dev server
cargo tauri dev       # starts cargo leptos watch, then opens the window
```

`cargo tauri dev` starts `cargo leptos watch` first. The watch runs the cli on http://127.0.0.1:3000, and the window shows the same pages through the plugin. For work in a browser, run `cargo leptos watch` alone and open that address.

## Make it yours

cargo-leptos ships a wizard that wraps cargo-generate. It asks for the display name, the bundle identifier and the author, with defaults from the project name. An empty author takes your git identity.

```bash
cargo leptos new --git https://github.com/nicmell/tauri-leptos
```

The wizard then asks to run `scripts/rename-app.sh`. That command is the rename, so answer yes. `cargo generate` takes the same template, and `--allow-commands` skips the question:

```bash
cargo generate --git https://github.com/nicmell/tauri-leptos \
    --name acme-app --allow-commands
```

In a clone, or in a repository made with GitHub's "Use this template", run the script yourself:

```bash
./scripts/rename-app.sh --name Acme --slug acme-app \
    --identifier com.acme.app --remove-self
```

`--author` defaults to your git identity. The script renames the crates, the bundle identifier, the Android package, the deb and systemd paths, and the docs in one pass. `--dry-run` shows the plan first. `--remove-self` deletes the script after the rename.

Then drop the demo and write your own:

| Delete | Then |
| --- | --- |
| `crates/ui/src/demo.rs` | point the route in `crates/ui/src/app.rs` at your page |
| `greet` in `src-tauri/src/lib.rs` and in `crates/ui/src/tauri_ipc.rs` | add your own commands and their bindings |
| `crates/ui/public/*.svg` | add your own static files |

The pipe stays, `crates/ui/public/pipe-worker.js` included, but its messages and its session are the demo's. Replace the variants of `ClientMessage` and `ServerMessage` in `crates/protocol/src/lib.rs`. Replace the tick and the echo in `crates/core/src/session.rs`. Then update their tests in `crates/protocol` and `crates/core/tests`.

Also run `cargo tauri icon <your.png>` for the icon set, and replace `LICENSE` with your app's.

cargo-generate does not copy symlinks. So in an app made with it, restore `AGENTS.md` with `ln -s CLAUDE.md AGENTS.md`.

## Where things go

| To add | Edit |
| --- | --- |
| a page or route | `crates/ui/src/app.rs`, with a module per page |
| a server function | any `#[server]` fn in `crates/ui`, served under `/api` |
| a setting | a field of `AppConfig` in `crates/ui/src/config.rs`, then its value where the cli and the Tauri app build it |
| a Tauri command or plugin | `src-tauri/src/lib.rs`, with a binding in `crates/ui/src/tauri_ipc.rs` |
| a message through the pipe | a variant in `crates/protocol/src/lib.rs`, with its answer in `crates/core/src/session.rs` |
| a page that uses the pipe | `tauri_leptos_core::page::Pipe`, as `PipeSection` in `crates/ui/src/demo.rs` uses it |
| a cli flag | `crates/app-cli/src/main.rs` |
| styles | `crates/ui/style/main.css` |
| static files | `crates/ui/public/` |

[docs/template-updates.md](docs/template-updates.md) covers how to pull later template changes into your app.

## Production build

```bash
cargo tauri build                 # desktop bundle
cargo tauri android build         # Android APK
./scripts/build-deb.sh            # Linux deb: the cli, the site and a systemd unit
```

Tauri builds run `cargo leptos build --release --frontend-only` first, and Tauri embeds the site in the app.

## Tests and quality

```bash
cargo nextest run --workspace --no-tests=pass
cargo clippy --workspace --all-targets
cargo clippy -p tauri-leptos-ui --features ssr
cargo clippy -p tauri-leptos-ui --features hydrate --target wasm32-unknown-unknown
cargo fmt --all && leptosfmt crates/ui/src
cargo deny check
bacon                             # watch loop (c/w/t/d)
```

CI runs all of the above on every PR. The conventions are in [CLAUDE.md](CLAUDE.md), and RustRover run configurations are in `.run/`.

## License

MIT-0 ([LICENSE](LICENSE)). It needs no attribution, so an app generated from this template replaces the file with its own.
