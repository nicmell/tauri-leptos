# Architecture

A pure-Rust application template: a Leptos SSR app with one axum router, served two ways. The cli serves it on an address you choose. The Tauri app serves it inside its window through [tauri-plugin-leptos-ssr](https://github.com/nicmell/tauri-plugin-leptos-ssr).

## Crate map

```
crates/ui        the Leptos app. Feature hydrate is the wasm client.
                 Feature ssr adds server::router(options, config): the
                 pages and server functions, without a fallback.
crates/core      the pipe from the page to a session, transport only:
                 frames of text or bytes. Feature hydrate holds the
                 page's end, the Tauri bindings and the web worker.
                 Feature ssr holds the /ws route and the Tauri app's
                 pipes, which run the session that the app brings.
crates/protocol  the demo's messages, their JSON encoding, and (feature
                 ssr) the session that answers them.
crates/app-cli   tauri-leptos-cli: the router plus the site files
                 (ServeDir) on --host/--port. cargo-leptos runs it as
                 the bin-package.
src-tauri        the Tauri app: tauri-plugin-log, the plugin with the
                 router, a window on the plugin's URL, the pipe
                 commands, and the demo command greet.
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

The plugin README describes the transport. The `leptos` scheme carries GET and HEAD requests. IPC carries requests with a body and event streams. On Android, the plugin's origin is `http://leptos.localhost`.

The Tauri dev build compiles the router but does not run it, because it forwards to the watch. So a change in `crates/ui` needs no new Tauri build, and `.taurignore` keeps `cargo tauri dev` from starting one. The pipes run in the Tauri app, so a change in `crates/core` or `crates/protocol` starts a new Tauri build.

## The pipe

The pipe connects the page to a session, a Rust task that answers the page. It carries frames, text or bytes, in both directions, and it does not read them. The app brings the session: an async function of the page's frames and of the frames that it sends back (`core::session`). The page opens the pipe with `core::page::Pipe::open`, which picks the transport at run time:

```
the page    Pipe::open checks for window.__TAURI__, which the Tauri app
            injects (app.withGlobalTauri)
browser     a module worker from /pipe-worker.js (core::worker) holds a
            websocket to /ws on the same server (core::server), and the
            page talks to the worker over a MessagePort
Tauri app   the page calls the commands pipe_open, pipe_post and
            pipe_close, and a Tauri channel brings the frames back
            (core::pipes, in the app)
both        the app's session answers (the demo's is in crates/protocol)
```

On every leg, the type of a JS value says what it is:

| JS value | Meaning | Direction |
| --- | --- | --- |
| string | a text frame | both ways |
| `ArrayBuffer` | a binary frame | both ways |
| `{ event: "connected" }` or `{ event: "disconnected" }` | the far end is reachable, or gone | to the page |
| `{ command: "connect" }` | connect again | to the worker |

The page hears `PipeEvent::Connected`, `Disconnected` or `Frame`. The demo sends JSON from `crates/protocol` in text frames, and its session echoes each binary frame as it is.

The page picks the transport, because only the page knows where it runs. In `cargo tauri dev`, the window gets its pages from the watch, which is the cli build. So a choice at build time fails there, but the window still has `window.__TAURI__`.

In a browser, `Pipe::open` starts a module worker from `/pipe-worker.js`, a site file in `crates/ui/public`. That script loads the app's own wasm and calls `worker_main` from `crates/core`. The page hands the worker one end of a `MessageChannel`. A `MessagePort` keeps its messages until its receiver listens, so the pipe needs no handshake while the worker loads. The websocket goes to `/ws` on the cli, so the pipe adds no port. `/ws` refuses a page from another origin with a 403.

A request to `/ws` without a readable `Origin` passes. A browser always sends one, and any other client can claim whatever it likes, so a demand for one only blocks honest tools. The reader and the writer of the socket are separate tasks. So a reader that waits for room in the session's queue never stops the session's frames from going out.

In the Tauri app, the pipe has no worker and no socket. `pipe_open` starts a session and returns the id of the pipe. The Tauri channel that the page passes to `pipe_open` carries text as JSON and bytes as raw data. `pipe_post` hands one frame to the session, and `pipe_close` ends it. The page waits for each call before the next, because Tauri does not promise the order of concurrent commands. [pipe-background.md](pipe-background.md) measures the channel in a background window on macOS.

The page decides when to connect again. After `Disconnected`, `Pipe::reconnect` asks the worker for a new socket, or opens a new pipe in the Tauri app. That pipe needs a new channel, because Tauri unregisters the callback of a channel when its pipe ends. A frame that the page sends while the pipe is disconnected is dropped. The demo connects again one second after `Disconnected`.

When the page drops its pipe, the session ends. In a browser, the worker stops and closes its socket. In the Tauri app, the page calls `pipe_close`.

A reload drops nothing. In a browser, the worker ends with its page. But a Tauri channel stays open after its page reloads. The app can still send to it, and the new page logs each message as an unknown callback. So when a page starts to load in a webview, or when its window closes, the app closes the pipes of that webview.

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
