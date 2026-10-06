# Architecture

A pure-Rust application template: a Leptos SSR app with one axum router, served two ways. The cli serves it on an address you choose. The Tauri app serves it inside its window through [tauri-plugin-leptos-ssr](https://github.com/nicmell/tauri-plugin-leptos-ssr).

## Crate map

```
crates/ui        the Leptos app. Feature hydrate is the wasm client.
                 Feature ssr adds server::router(options, config): the
                 pages and server functions, without a fallback.
crates/core      the pipe from the page to a session, transport only:
                 frames of text or bytes over a websocket to /ws.
                 Feature hydrate holds the page's end, the socket on
                 the page's main thread and the web worker between
                 them. Feature ssr holds the /ws route, which runs the
                 session that the app brings.
crates/protocol  the demo's messages, their JSON encoding, and (feature
                 ssr) the session that answers them.
crates/app-cli   tauri-leptos-cli: the router plus the site files
                 (ServeDir) on --host/--port. cargo-leptos runs it as
                 the bin-package.
src-tauri        the Tauri app: tauri-plugin-log, the plugin with the
                 router, a window on the plugin's URL, and the demo
                 command greet.
```

## Two hosts, one router

`crates/ui/src/server.rs` builds the router. Both hosts call it and add their own fallback for the site files.

```
dev      cargo leptos watch   the cli on 127.0.0.1:3000, live reload on 3001
         cargo tauri dev      the window loads leptos://localhost/, and the
                              plugin forwards each request to the watch
release  the cli              the router and target/site on --host/--port
         the Tauri app        the plugin dispatches to the router in process,
                              with the site that Tauri embeds in the app
```

The plugin README describes the transport. The `leptos` scheme carries GET and HEAD requests. IPC carries requests with a body, event streams and websockets. On Android, the plugin's origin is `http://leptos.localhost`.

The Tauri dev build compiles the router but does not run it, because it forwards to the watch, the pipe's websocket included. So a change in `crates/ui`, `crates/core` or `crates/protocol` needs no new Tauri build, and `.taurignore` keeps `cargo tauri dev` from starting one.

## The pipe

The pipe connects the page to a session, a Rust task that answers the page. It carries frames, text or bytes, in both directions, and it does not read them. The app brings the session: an async function of the page's frames and of the frames that it sends back (`core::session`). `/ws` (`core::server`) runs one session per websocket. The page opens the pipe with `core::page::Pipe::open`, and the same three parts carry it in both hosts:

```
page        Pipe::open, on the page's main thread
worker      a module worker from /pipe-worker.js (core::worker), which
            relays every value between two MessagePorts
socket      a websocket to /ws on the page's main thread (core::socket)
browser     the browser's own websocket, to /ws on the cli
Tauri app   tauri-plugin-leptos-ssr carries the websocket over IPC: to
            the router's /ws in process, or to the watch in dev
both        the app's session answers (the demo's is in crates/protocol)
```

On both ports of the worker, the type of a JS value says what it is:

| JS value | Meaning | Direction |
| --- | --- | --- |
| string | a text frame | both ways |
| `ArrayBuffer` | a binary frame | both ways |
| `{ event: "connected" }` or `{ event: "disconnected" }` | the socket opened, or it closed | to the page |
| `{ command: "connect" }` | open a new socket | from the page |

The page hears `PipeEvent::Connected`, `Disconnected` or `Frame`. The demo sends JSON from `crates/protocol` in text frames, and its session echoes each binary frame as it is.

The socket sits on the main thread, because workers have no Tauri IPC, and the plugin's `WebSocket` class exists only in the page's main frame. The worker sits between the page and the socket in both hosts. In a background window, its timers keep their rate, while timers on the main thread drop to one run per second ([pipe-background.md](pipe-background.md)). So work on a timer can move into the worker without a change to the path.

`/pipe-worker.js` is a site file in `crates/ui/public`. It loads the app's own wasm and calls `worker_main` from `crates/core` with the two ports. A `MessagePort` keeps its messages until its receiver listens, so the pipe needs no handshake while the worker loads.

The socket's URL is `/ws` on the page's origin: `ws:` for an `http:` page, `wss:` for `https:`, and the page's own scheme otherwise. On macOS, the Tauri app's pages are on `leptos://localhost`, and the plugin takes that URL. The pipe adds no port.

`/ws` refuses a page from another origin with a 403. A request without a readable `Origin` passes. A browser always sends one, and any other client can claim whatever it likes, so a demand for one only blocks honest tools. The plugin's socket sends no `Origin`. The reader and the writer of the socket are separate tasks. So a reader that waits for room in the session's queue never stops the session's frames from going out.

Only the page connects the pipe again. After `Disconnected`, `Pipe::reconnect` asks for a new socket. A frame that the page sends while the pipe is disconnected is dropped. The demo connects again one second after `Disconnected`.

When the page drops its pipe, the worker stops and the socket closes, so the session ends. In the Tauri app, the plugin also closes the sockets of a webview at two moments: a page starts to load in it, or its window closes.

## Ports

| Port | Use |
| --- | --- |
| 3000 | the cli: the watch server, and the default of `--host` and `--port` |
| 3001 | the cargo-leptos live-reload socket |

The leptos `site-addr`, the cli defaults and the Tauri `devUrl` must agree. Two tests check them: one in `crates/app-cli/src/main.rs` and one in `src-tauri/tests/config.rs`.

## App config

`AppConfig` in `crates/ui/src/config.rs` is empty for now. The router provides it as Leptos context to server rendering and to server functions. Both entry points pass `AppConfig::default()`. A new field gets its value where an entry point builds the struct, for example from a cli flag.

## Asset names and site builds

- `.cargo/config.toml` sets `LEPTOS_OUTPUT_NAME` at compile time. Leptos derives the asset URLs from it, for example `/pkg/tauri-leptos.wasm`. Only cargo-leptos builds set it on their own. Without it, plain cargo builds ask for the wasm-bindgen name `_bg.wasm`, and hydration fails. The Tauri app and the deb binary are plain cargo builds.
- Dev cargo-leptos builds compile Leptos with other flags. A page from such a build hydrates only against its own watch server. So a server built with plain cargo serves a release site, from `cargo leptos build --release --frontend-only`. Tauri runs that command in `beforeBuildCommand`.
- `bin-target-dir = "target/server"` gives the watch its own target directory. Without it, the watch and the Tauri dev build rebuild each other's dependencies.
- `hash-files` stays off, because the plugin and `crates/ui/public/pipe-worker.js` look for the site files under their plain names.

## Dev workflows

```bash
cargo tauri dev              # the watch, then the window
cargo leptos watch           # the watch alone, for a browser on http://127.0.0.1:3000
cargo leptos serve --release # the release cli, as a deployment runs it
cargo tauri android dev      # Android, see install/android.md
```

## Standalone server

The cli takes its address only from `--host` and `--port`. Leptos reads the site root from `LEPTOS_SITE_ROOT`, with the default `target/site`. The deb sets it in its systemd unit. See [install/linux-systemd.md](install/linux-systemd.md).

## Out of scope for now

End-to-end tests and iOS.
