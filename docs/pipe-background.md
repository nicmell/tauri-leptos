# Pipe messages in the background

On macOS, a page in a hidden Tauri window gets less processing time. Earlier tests showed that a websocket on the main thread of the page then lagged, but a websocket in a web worker did not. In the Tauri app, the pipe now delivers its messages on the main thread, over Tauri channels. This report measures the lag of those messages in a background window.

The measurement ran on 2026-10-05, on an Apple M5 with macOS 26.6.2 (25G83). It used Tauri 2.12.0, wry 0.57.0 and the `feat/pipe` branch at `87d605f`, with the temporary changes that the next section lists.

The pipe changed after this measurement. It no longer has `/pipe.js`. In the Tauri app, the pipe is now a websocket that tauri-plugin-leptos-ssr carries over its commands, with the worker between the page and the socket. Those commands reach Rust through the app's main thread, so finding 4 applies to them as it does to the channels.

## Setup

The measurement ran the Tauri dev build, `target/debug/tauri-leptos`, against `cargo leptos watch` on `127.0.0.1:3000`. These changes were temporary and are not in the repository:

- The session sends a tick every 100 ms instead of every second. Each tick carries `sent_at`, its send time in milliseconds since the Unix epoch.
- The session logs the text of every echo that it receives, together with its own receive time.
- `/ws` accepts every origin, because the page in the Tauri window opens its sockets to the watch.
- The window loads a measurement page, `throttle.html`, instead of the demo. The window is 420 by 320 points and opens without focus.
- A thread in the Tauri app runs the phases. The window stays visible for 30 seconds. Then the thread minimizes the window or hides the app. After the background phase, the app quits without a restore.
- `THROTTLE_ACTION` selects minimize or hide. `THROTTLE_POLICY` sets the background throttling policy of the window.

## What the page measures

The page opens five paths at the same time, so every path sees the same conditions:

| Path | How the messages reach the page |
| --- | --- |
| `chan` | a Tauri `Channel` that the page passes to `pipe_open` |
| `pipe` | the channel pipe from `/pipe.js`: a Tauri channel, then a `MessagePort` |
| `ws-main` | a websocket to `/ws` on the watch, on the main thread |
| `ws-worker` | a websocket in a web worker, which posts each tick to the main thread |
| `timer` | a `setInterval` of 100 ms on the main thread, the control for throttling |

The worker also measures its own socket (`worker:ws`) and its own interval of 100 ms (`worker:timer`).

For each message, the receiver records the gap since the previous message, and the delay: `Date.now()` minus `sent_at`. The page, the worker and both sessions run on the same Mac, so their clocks agree. Once a second, each receiver reports its message count, its largest gap and its largest delay. The main thread reports through `pipe_post`, and the worker reports through its own socket. The sessions log each report with their receive time.

In the visible phase, every path received 9.7 to 9.8 messages per second. The gaps were 109 ms or less, and the delays were 10 ms or less.

## Results

| Run | Policy | Background | Length |
| --- | --- | --- | --- |
| 1 | default | window minimized | 120 s |
| 2 | default | app hidden | 120 s |
| 3 | `Suspend` | window minimized | 120 s |
| 4 | default | window minimized | 390 s |

In runs 1 to 3, `document.visibilityState` became `hidden` within 0.6 s of the action. In run 4, the page reported `hidden` from its first report, so that run has no visible baseline.

Messages per second in the background phase. The sessions send 10 per second.

| Path | Run 1 | Run 2 | Run 3 | Run 4 |
| --- | --- | --- | --- | --- |
| `chan` | 10.0 | 10.0 | 10.0 | 10.0 |
| `pipe` | 10.0 | 10.0 | 10.0 | 10.0 |
| `ws-main` | 10.0 | 10.0 | 10.0 | 10.0 |
| `ws-worker` | 10.0 | 10.0 | 10.0 | 10.0 |
| `worker:ws` | 10.0 | 10.0 | 10.0 | 10.0 |
| `timer` | 1.1 | 1.0 | 1.1 | 1.0 |
| `worker:timer` | 9.9 | 9.8 | 9.9 | 9.9 |

The largest gap between two messages in the background phase, in ms. The median gap was 101 or 102 ms on every message path.

| Path | Run 1 | Run 2 | Run 3 | Run 4 |
| --- | --- | --- | --- | --- |
| `chan` | 564 | 118 | 608 | 606 |
| `pipe` | 564 | 116 | 608 | 606 |
| `ws-main` | 105 | 105 | 192 | 169 |
| `ws-worker` | 111 | 106 | 192 | 167 |
| `worker:ws` | 110 | 105 | 192 | 162 |
| `timer` | 1001 | 1001 | 1002 | 1013 |
| `worker:timer` | 105 | 106 | 109 | 132 |

The largest delay from `sent_at` to the receiver in the background phase, in ms:

| Path | Run 1 | Run 2 | Run 3 | Run 4 |
| --- | --- | --- | --- | --- |
| `chan` | 465 | 11 | 508 | 508 |
| `pipe` | 465 | 13 | 508 | 507 |
| `ws-main` | 6 | 4 | 93 | 71 |
| `ws-worker` | 11 | 6 | 93 | 67 |
| `worker:ws` | 11 | 5 | 93 | 64 |

These are all the gaps above 150 ms on a message path:

- In runs 1, 3 and 4, the channel paths paused twice right after the minimize. The first pause came at 0.3 to 0.4 s, for 326 to 428 ms. The second came at 1.3 to 1.4 s, for 564 to 608 ms.
- In run 3, the socket paths also paused for 192 ms at 1.4 s.
- In run 4, the channel paths paused for 448 ms at 185.9 s. Single paths paused for 162 to 190 ms at 119 s, 128 s and 191 s. The measurement did nothing at those times.
- In run 2, no gap was above 118 ms.

The reports from the page to Rust over `pipe_post` arrived at most 8 ms late in the visible phases and in run 2. In the minimize runs, they arrived up to 278 ms late, and the largest delays came 0.3 to 0.4 s after the minimize.

## Findings

1. A hidden window on macOS throttles the timers on the main thread of the page to one run per second. The timers of a worker keep their rate.
2. A hidden window does not throttle message delivery. Every path kept 10 messages per second for up to 390 s, with the default policy and with `Suspend`.
3. The lag of a main-thread websocket from the earlier tests did not occur. Main-thread code that depends on timers lags, for example heartbeats, batching, reconnect delays or polling. That is a possible cause, but this measurement did not test it.
4. The channel pipe pauses in both directions while the main thread of the Tauri app is busy, for example during the minimize animation. The pauses lasted 0.3 to 0.6 s, and the messages arrived in order after them. In the same runs, the socket paths paused for 0.19 s or less.
5. The `Suspend` policy did not suspend the page within 120 s, and the default policy did not within 390 s.

Finding 4 follows from how a channel delivers. For JSON under 8 KB, `Channel::send` evaluates JavaScript in the webview (tauri 2.12.0, `src/ipc/channel.rs`), and the main thread of the app carries every evaluation. On macOS, a command call from the page also reaches Rust through the main thread of the app. A websocket message goes from the network process to the web content process, and the main thread of the app has no part in it.

## Not covered

- Android. The measurement stopped before its first background phase.
- A background phase longer than 390 s, a window that other windows cover, another Space, display sleep and a locked screen.
- Release builds. The measurement used the dev build, with the pages from the watch.
- Other work on the main thread of the app: open menus, live resize, native dialogs and slow synchronous commands. By the mechanism of finding 4, each of them can pause the channel pipe.
- Channel messages of 8 KB or more. Tauri delivers them through an extra IPC request.
- A main-thread websocket in a browser tab, which is the cli mode.

## Possible fixes

For the timers on the main thread (finding 1):

- Keep work that depends on timers off the main thread of the page. The timers of the session run in Rust, in the Tauri app or in the cli, so the tick keeps its rate. Work that the page must time itself can run in a worker.
- Set `backgroundThrottling` to `disabled` on the window (`WebviewWindowBuilder::background_throttling`). On macOS 14 and later, this sets the `inactiveSchedulingPolicy` of WebKit to none. Android does not support it, and this measurement did not test it.

For the pauses of the channel pipe (finding 4):

- Accept them. They lasted less than a second, they came from work on the main thread of the app, and no message was lost.
- Keep the main thread of the app free. A command without `async` runs on the main thread on macOS. `pipe_post` is such a command, but it only hands a message to the session.
- Send fewer, larger channel messages, for example a batch of session messages per `send`. That reduces the number of evaluations, but a busy main thread still delays each batch.
- Move the Tauri side of the pipe into a worker. A worker has no Tauri IPC: it has no `__TAURI_INTERNALS__`, and on Android the IPC needs the main frame of the page. So the worker needs a socket again: the loopback server of PR #16. That server brings back its port, its origin list and its Android network security configuration.

## Repeat the measurement

The appendix holds the page and the worker. To repeat the measurement on a Mac:

1. On `feat/pipe`, make the temporary changes from the setup section.
2. Copy `throttle.html` and `throttle-worker.js` to `crates/ui/public/`.
3. In the repository root, start `cargo leptos watch`.
4. In a second terminal, build the Tauri app with `cargo build -p tauri-leptos`.
5. Run `THROTTLE_ACTION=minimize ./target/debug/tauri-leptos`.
6. For another policy, add `THROTTLE_POLICY=suspend` or `THROTTLE_POLICY=disabled`.
7. Group the `ECHO` lines of the app log and the watch log by phase. The `PHASE` lines in the app log give the start of each phase.

## Appendix: the measurement page and worker

`crates/ui/public/throttle.html`:

```html
<!doctype html>
<html>
<head><meta charset="utf-8"><title>throttle</title></head>
<body>
<pre id="out">throttle measurement</pre>
<script type="module">
const WS_URL = 'ws://127.0.0.1:3000/ws';
const out = document.getElementById('out');
const paths = {};
let report = () => {};
let lastReport = Date.now();

function record(name, sentAt) {
  const now = Date.now();
  const p = (paths[name] ??= { n: 0, maxGap: 0, maxDelay: 0, last: now });
  p.maxGap = Math.max(p.maxGap, now - p.last);
  p.last = now;
  p.n++;
  if (sentAt) p.maxDelay = Math.max(p.maxDelay, now - sentAt);
  maybeReport(now);
}

function maybeReport(now) {
  if (now - lastReport < 1000) return;
  lastReport = now;
  const lines = [];
  for (const [name, p] of Object.entries(paths)) {
    const gap = Math.max(p.maxGap, now - p.last);
    lines.push(`R main ${name} t=${now} vis=${document.visibilityState} foc=${document.hasFocus()} n=${p.n} maxgap=${gap} maxdelay=${p.maxDelay}`);
    p.n = 0;
    p.maxGap = 0;
    p.maxDelay = 0;
  }
  for (const line of lines) report(line);
  out.textContent = lines.join('\n');
}

const tick = (json) => {
  const e = JSON.parse(json);
  return e.type === 'received' && e.message.type === 'tick' ? e.message : null;
};

const { invoke, Channel } = window.__TAURI__.core;
const events = new Channel();
events.onmessage = (json) => {
  const t = tick(json);
  if (t) record('chan', t.sent_at);
};
const id = await invoke('pipe_open', { events });
report = (text) =>
  invoke('pipe_post', { id, data: JSON.stringify({ type: 'echo', text }) }).catch(() => {});
document.addEventListener('visibilitychange', () =>
  report(`V main t=${Date.now()} vis=${document.visibilityState}`),
);

const { open } = await import('/pipe.js');
const port = open();
port.onmessage = ({ data }) => {
  const t = tick(data);
  if (t) record('pipe', t.sent_at);
};

const ws = new WebSocket(WS_URL);
ws.onmessage = ({ data }) => {
  const m = JSON.parse(data);
  if (m.type === 'tick') record('ws-main', m.sent_at);
};

const worker = new Worker('/throttle-worker.js');
worker.onmessage = ({ data }) => record('ws-worker', data);

setInterval(() => record('timer', 0), 100);
</script>
</body>
</html>
```

`crates/ui/public/throttle-worker.js`:

```js
const ws = new WebSocket('ws://127.0.0.1:3000/ws');
const socket = { n: 0, maxGap: 0, maxDelay: 0, last: Date.now() };
const timer = { n: 0, maxGap: 0, last: Date.now() };
let lastReport = Date.now();

function maybeReport(now) {
  if (now - lastReport < 1000 || ws.readyState !== 1) return;
  lastReport = now;
  const send = (text) => ws.send(JSON.stringify({ type: 'echo', text }));
  send(`R worker ws t=${now} n=${socket.n} maxgap=${Math.max(socket.maxGap, now - socket.last)} maxdelay=${socket.maxDelay}`);
  send(`R worker timer t=${now} n=${timer.n} maxgap=${Math.max(timer.maxGap, now - timer.last)} maxdelay=0`);
  socket.n = socket.maxGap = socket.maxDelay = 0;
  timer.n = timer.maxGap = 0;
}

ws.onmessage = ({ data }) => {
  const m = JSON.parse(data);
  if (m.type !== 'tick') return;
  const now = Date.now();
  socket.maxGap = Math.max(socket.maxGap, now - socket.last);
  socket.last = now;
  socket.n++;
  socket.maxDelay = Math.max(socket.maxDelay, now - m.sent_at);
  postMessage(m.sent_at);
  maybeReport(now);
};

setInterval(() => {
  const now = Date.now();
  timer.maxGap = Math.max(timer.maxGap, now - timer.last);
  timer.last = now;
  timer.n++;
  maybeReport(now);
}, 100);
```
