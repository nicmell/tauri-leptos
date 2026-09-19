#!/usr/bin/env node
// Hydration smoke test over raw CDP: boots the frontend-enabled server,
// loads the page in headless Chrome, clicks the server-fn and WebSocket
// demo buttons, and asserts both answers appear in the DOM.
// Prerequisites: `cargo leptos build --release` (dev builds carry
// hot-reload instrumentation only the dev-server can hydrate against)
// and `cargo build -p tauri-leptos-cli --features frontend`.
import { spawn, execSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const PORT = 3777;
const SERVER = process.env.SMOKE_SERVER ?? "target/debug/tauri-leptos-cli";
const CHROME =
  process.env.CHROME ??
  (process.platform === "darwin"
    ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    : "google-chrome");

const appDir = mkdtempSync(join(tmpdir(), "tl-smoke-"));
const procs = [];
const cleanup = () => {
  for (const p of procs) p.kill("SIGKILL");
  try {
    rmSync(appDir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  } catch {
    // chrome may still be flushing its profile; leftover tmp dirs are fine
  }
};
const browserLog = [];
let evaluate = null; // assigned once the CDP session is up
const fail = async (msg) => {
  console.error(`FAIL: ${msg}`);
  for (const line of browserLog.slice(-25)) console.error(`  [browser] ${line}`);
  if (evaluate) {
    const body = await evaluate("document.body.innerHTML.slice(0, 600)").catch(() => "?");
    console.error(`  [dom] ${body}`);
  }
  cleanup();
  process.exit(1);
};

const wait = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(desc, fn, timeoutMs = 15000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await fn().catch(() => false)) return;
    await wait(300);
  }
  await fail(`timeout waiting for ${desc}`);
}

// 1. server
procs.push(
  spawn(
    SERVER,
    ["serve", "--app-dir", appDir, "--site-root", "target/site", "--listen", `127.0.0.1:${PORT}`],
    { stdio: "inherit" },
  ),
);
await until("server", async () => (await fetch(`http://127.0.0.1:${PORT}/api/hello`)).ok);
const home = await (await fetch(`http://127.0.0.1:${PORT}/`)).text();
if (home.includes("no frontend in this build"))
  fail("server built without the frontend feature - run: cargo build -p tauri-leptos-cli --features frontend");

// 2. headless chrome with CDP
const cdpPort = 9333;
procs.push(
  spawn(
    CHROME,
    [
      "--headless=new",
      "--no-sandbox",
      "--disable-gpu",
      `--remote-debugging-port=${cdpPort}`,
      `--user-data-dir=${join(appDir, "chrome")}`,
    ],
    { stdio: "ignore" },
  ),
);
await until("cdp", async () => (await fetch(`http://127.0.0.1:${cdpPort}/json/version`)).ok);

// note: newer headless Chrome ignores the url param here — navigate below
const target = await (
  await fetch(`http://127.0.0.1:${cdpPort}/json/new`, { method: "PUT" })
).json();
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = reject;
});

let msgId = 0;
const pending = new Map();
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
    return;
  }
  // collect page console + errors for failure diagnostics
  if (msg.method === "Runtime.consoleAPICalled") {
    browserLog.push(
      `${msg.params.type}: ${msg.params.args.map((a) => a.value ?? a.description ?? "").join(" ")}`,
    );
  } else if (msg.method === "Runtime.exceptionThrown") {
    browserLog.push(`exception: ${msg.params.exceptionDetails.text} ${msg.params.exceptionDetails.exception?.description ?? ""}`);
  } else if (msg.method === "Network.responseReceived") {
    const r = msg.params.response;
    if (r.status >= 400) browserLog.push(`http ${r.status}: ${r.url}`);
  }
};
const command = (method, params = {}) =>
  new Promise((resolve) => {
    const id = ++msgId;
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });
evaluate = async (expression) =>
  (await command("Runtime.evaluate", { expression, returnByValue: true })).result?.result?.value;

await command("Runtime.enable");
await command("Network.enable");
await command("Page.navigate", { url: `http://127.0.0.1:${PORT}/` });

// 3. wait for the page, then for the explicit hydration marker set by
// ui::hydrate() — no interaction before it: poking the DOM while tachys
// walks it for hydration corrupts the node matching (CI runners
// instantiate debug wasm slowly enough to hit that window).
await until(
  "page render",
  async () => (await evaluate("document.querySelector('h1')?.textContent")) === "Welcome to Tauri + Leptos",
);
await until(
  "hydration marker",
  async () => (await evaluate("document.body.dataset.hydrated")) === "true",
  90000,
);
const click = (label) =>
  evaluate(
    `[...document.querySelectorAll('button')].find(b => b.textContent.trim() === ${JSON.stringify(label)})?.click(), true`,
  );

await click("Server fn greet");
await until(
  "server fn answer",
  async () => (await evaluate("document.body.textContent")).includes("rendered by a server function"),
  30000,
);
await click("WebSocket echo");
await until(
  "websocket echo",
  async () => (await evaluate("document.body.textContent")).includes("echo: ping from the ui"),
  30000,
);

console.log("smoke OK: SSR page hydrated, server fn and websocket answered");
cleanup();
process.exit(0);
