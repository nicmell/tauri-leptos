# Running sc-app2 on a Raspberry Pi

End-to-end setup for running sc-app2 on a Raspberry Pi (validated on a Pi 5,
Debian 13 "trixie", aarch64). It covers the audio stack (JACK + scsynth +
StrudelDirt as systemd services), the one Linux-specific gotcha that breaks the
SHM scopes (`RemoveIPC`), and building/running the app itself.

Companion docs: `CLAUDE.md` (architecture), `scope.md` (the SHM scope pipeline).

## 0. What runs where

```
jackd.service        ALSA <-> JACK (the audio device, e.g. HiFiBerry)
  └─ scsynth.service   scsynth on UDP 57110  (the synth server + SHM scope buffers)
       ├─ strudeldirt.service   sclang + SuperDirt on UDP 57120  (/dirt/* events)
       └─ sc-app2 server        HTTP/WS on :3000  (the OSC bridge + dashboard)
            └─ browser           http://<pi>:1420 (dev) or :3000 (built)
```

The three audio pieces run as **systemd services** so they survive reboots and
logouts; the app is started on demand (or could be a fourth service).

Versions on the reference Pi: SuperCollider **3.14.1** (built with shared-memory
support), jackd **1.9.22** (JACK2/jackdmp), Node **24** (via nvm), Rust (rustup),
yarn **4.14.1** (via corepack).

## 1. Prerequisites

Toolchains (install once, any version manager is fine):

- **Node 20+** and **corepack** (ships with Node): `corepack enable` puts the
  repo-pinned `yarn@4` on PATH.
- **Rust** (rustup) for the backend.
- **SuperCollider 3.14.x** with `scsynth`/`sclang` on PATH (`/usr/local/bin` if
  built from source). Must include the server shared-memory interface (the
  stock build does — verify with `strings $(command -v scsynth) | grep
  SuperColliderServer`). Plus **sc3-plugins** for global effects
  (delay/reverb): `sudo apt install supercollider-sc3-plugins` or build them.
- **JACK2** (`jackd`) wired to your audio device.

System packages for building the Tauri/Rust backend on Debian/RPi OS:

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

`libwebkit2gtk-4.1-dev` pulls in the `glib`/`gtk` dev files the `tauri` crate
links against. (Alternatively, the `headless-setup` branch makes `tauri`
optional behind a cargo feature so `--no-default-features` builds the server
with **no** WebKit/GTK deps — handy on a headless Pi. Until that merges, install
the libs above.)

## 2. Project checkout + JS deps

```bash
corepack enable                 # yarn 4 on PATH
git submodule update --init     # strudeldirt + sc-app submodules
yarn install
```

`yarn deps` (Dirt-Samples / Vowel into `deps/`) is **not** required on the Pi if
you install the SuperCollider quarks instead — see §4. It is only used by the
repo's `yarn osc` dev workflow.

## 3. The audio stack as systemd services

Reference unit files live in `scripts/rpi/`. Edit `User=`/`Group=` and paths to
match your checkout, then install them.

### jackd

Run JACK against your card (this Pi uses a HiFiBerry). Either your distro's
`jackd` service or a unit like:

```
ExecStart=/usr/bin/jackd -R -d alsa -d hw:sndrpihifiberry -p1024 -n3 -i8 -o8
```

### scsynth (`scripts/rpi/scsynth.service`)

```bash
sudo cp scripts/rpi/scsynth.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now scsynth.service
```

Key flags (`-u 57110 -i 8 -o 8 -a 8192 -b 262144 -m 262144 -w 2048 -n 32768
-l 32`): port 57110; `-l 32` (maxLogins) so the bridge, sclang/StrudelDirt and
extras each get a distinct clientID + node-id block; and `-m 262144` (256 MB RT
memory), `-w 2048` (wire buffers), `-n 32768` (max nodes) sized for SuperDirt's
global effects and complex plugin synthdefs — under-sizing these produces
`alloc failed, increase server's RT memory` and `exceeded number of interconnect
buffers` at runtime.

## 4. The `RemoveIPC` gotcha (REQUIRED for scopes — and stability)

**This is the single most important step on Linux.** systemd-logind defaults to
`RemoveIPC=yes`, which deletes all POSIX shared memory (and SysV IPC) owned by a
*regular* user (UID ≥ 1000) the moment that user's **last login session ends**.

Because the services above run as a regular user (`nick`), this silently reaps:

- `/dev/shm/SuperColliderServer_57110` — scsynth's SHM scope buffers, so every
  `<sc-scope>` goes dark (the bridge can't `mmap` the segment); and
- the JACK shm registry — after which scsynth can't find jackd, auto-spawns a
  doomed jackd, and crash-loops.

Fix: make the service user **linger** so logind never considers it logged out,
then restart the audio stack so the segments are recreated:

```bash
sudo loginctl enable-linger nick
sudo systemctl restart jackd.service scsynth.service
# verify (no auto-start), should LIST ports and the segment should exist:
JACK_NO_START_SERVER=1 jack_lsp
ls -l /dev/shm/SuperColliderServer_57110
```

(Alternatives: run the services as a *system* user with UID < 1000, which is
exempt from `RemoveIPC`; or set `RemoveIPC=no` in `/etc/systemd/logind.conf`.)
See `scope.md` §7 for the full explanation.

## 5. SuperCollider quarks (SuperDirt's dependencies)

StrudelDirt is the SuperDirt fork; it needs the **Vowel** quark for its vowel
formant module (without it, SuperDirt init dies with
`Message 'formLib' not understood` / `Class 'Vowel' not found`). Install both
quarks from sclang once:

```supercollider
Quarks.install("https://github.com/supercollider-quarks/Vowel.git");
Quarks.install("https://github.com/daslyfe/StrudelDirt.git");
```

This clones them into `~/.local/share/SuperCollider/downloaded-quarks/` and adds
them to `~/.config/SuperCollider/sclang_conf.yaml`. Recompile the class library
(restart sclang) afterwards. Confirm both are on the include path:

```bash
grep -E 'StrudelDirt|Vowel' ~/.config/SuperCollider/sclang_conf.yaml
```

## 6. Point sclang at the systemd scsynth (per-user `startup.scd`)

So that **every** sclang session (the StrudelDirt service and any interactive
use) defaults to the systemd scsynth instead of booting its own, install the
per-user startup file. sclang runs it on every launch, before any script passed
on the command line:

```bash
cp scripts/rpi/startup.scd ~/.config/SuperCollider/startup.scd
```

It configures `s` (`Server.default`) to attach to `127.0.0.1:57110` with
allocator options matched to `scsynth.service`'s flags (`-i 8 -o 8 -a 8192
-b 262144 -m 262144 -w 2048 -n 32768 -l 32`) — keep the two in sync if you change the unit. Never
`s.boot` from sclang; the service owns scsynth's lifecycle (attach with
`s.doWhenBooted`).

## 7. StrudelDirt as a service (`scripts/rpi/strudeldirt.service`)

Mounts SuperDirt on UDP 57120, attaching to the running scsynth (it never boots
its own). The startup script is `scripts/rpi/strudeldirt-attach.scd`; the server
options/address come from the per-user `startup.scd` installed in §6, so this
script just mounts SuperDirt.

```bash
sudo cp scripts/rpi/strudeldirt.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now strudeldirt.service
# verify:
journalctl -u strudeldirt.service -n 30 --no-pager   # expect: STRUDELDIRT_READY on UDP 57120
ss -lunp | grep 57120                                 # sclang listening
```

Notes:

- The unit runs `tail -f /dev/null | sclang …` so sclang's stdin stays open and
  it doesn't quit on EOF (there is no TTY under systemd).
- It runs as `User=nick` so sclang reads that user's quark config. linger (§4)
  keeps it valid.
- The unit sets `LimitRTPRIO=95` so sclang's scheduler can run at realtime
  priority. Without it sclang logs `Couldn't set realtime scheduling priority …
  Operation not permitted` — harmless (SuperDirt still runs), but the limit
  tightens language-side event timing.
- Sample playback (`bd`, `sn`, `hh`, …) needs a sample library. Clone
  Dirt-Samples to the path the unit's `SC_APP_DIRT_SAMPLES` points at
  (`/home/nick/Dirt-Samples` by default — ~390 MB, 200+ banks):

  ```bash
  git clone --depth 1 https://github.com/tidalcycles/dirt-samples.git /home/nick/Dirt-Samples
  ```

  SuperDirt loads them asynchronously after `STRUDELDIRT_READY` (the log shows
  `N existing sample banks`). Without the samples, only synth-based events sound.
  To use a different location, edit `Environment=SC_APP_DIRT_SAMPLES=…/*` in the
  unit (the trailing `/*` glob is expanded by SuperDirt, not the shell).

## 8. Run the app

With the audio services up, start the bridge + frontend. The app's bridge
forwards `/[sngbcdpu]_*` to scsynth (57110) and `/dirt/*` to StrudelDirt
(57120), and reads the SHM scope segment directly.

```bash
yarn dev:full      # Vite on :1420 + the Rust server on :3000 (/api,/ws proxied)
```

Open `http://<pi-address>:1420`. (For a production build: `yarn build` then run
the server; on the `headless-setup` branch, `yarn serve:headless` serves the
built `dist/` with no GTK deps.)

### Reaching the app from other machines on the LAN

By default Vite binds `127.0.0.1`, so the dashboard is only reachable on the Pi
itself. Bind it to all interfaces with `--host`:

```bash
yarn dev:full:lan        # = vite --host (0.0.0.0:1420) + the Rust server
```

Then browse to `http://<pi-ip>:1420/` from any LAN machine (find the IP with
`hostname -I`). **Only port 1420 needs to be reachable** — Vite proxies `/api`
and `/ws` to `127.0.0.1:3000` server-side on the Pi, so the Rust server can stay
bound to loopback.

Caveats:

- **Use the IP** (e.g. `http://192.168.178.100:1420/`). Vite 6 blocks requests
  whose `Host` header is an unknown *hostname* (DNS-rebinding protection); IPs
  are always allowed. To use a name like `raspberrypi.local`, add
  `server.allowedHosts: [...]` (or `true`) to `vite.config.ts`.
- This exposes the unauthenticated control API to the whole LAN — fine on a
  trusted home network, but don't put it on an untrusted one.
- **Serving the built app directly on :3000** (no Vite, e.g. a production
  appliance) needs the Rust listener to bind `0.0.0.0` instead of `127.0.0.1`
  (`core/router/mod.rs::listen`); that's a code change (a configurable bind
  host), not just a flag. In dev, the Vite `--host` route above avoids it.

## 9. Verification checklist

```bash
systemctl is-active jackd.service scsynth.service strudeldirt.service   # active x3
JACK_NO_START_SERVER=1 jack_lsp | head                                  # lists ports
systemctl show scsynth.service -p NRestarts                             # NRestarts=0 (no crash loop)
ls -l /dev/shm/SuperColliderServer_57110                                # segment present
loginctl show-user $USER | grep Linger                                  # Linger=yes
ss -lunp | grep 57120                                                   # StrudelDirt listening
```

In the app: drop a plugin with `<sc-scope bus="0">` on the dashboard — it should
render the master-out waveform (proves the SHM scope path), and Strudel patterns
should sound (proves the `/dirt/*` path).

## 10. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Scopes stay dark; `/dev/shm/SuperColliderServer_57110` missing | `RemoveIPC` reaped the segment on logout | §4 — `enable-linger` + restart scsynth |
| scsynth crash-loops, "Audio device hw:0 cannot be acquired" | can't reach jackd (reaped JACK shm) → auto-spawns a doomed jackd | stop both, `rm -f /dev/shm/jack-shm-registry /dev/shm/jack_sem.*`, restart jackd then scsynth; then §4 |
| `jack_lsp` says "server not running" though jackd is up | JACK shm registry reaped/stale | restart jackd.service (and §4) |
| StrudelDirt: `Class 'Vowel' not found` / `formLib` error | Vowel quark missing | §5 — install the Vowel quark, restart sclang |
| StrudelDirt: "Could not open UDP port 57120" | a previous sclang still holds it | `pkill -x sclang`, then restart strudeldirt.service |
| StrudelDirt: "Couldn't set realtime scheduling priority … not permitted" | the unit lacks the RTPRIO rlimit | harmless; add `LimitRTPRIO=95` to strudeldirt.service (already in the shipped unit) and restart |
| scsynth: "alloc failed, increase server's RT memory" | `-m` too small | set `-m 262144` in scsynth.service (the shipped unit does) and restart |
| scsynth: "exceeded number of interconnect buffers" | `-w` too small (defaults to 64) | add `-w 2048` to scsynth.service and restart |
| scsynth: "UGen 'MouseX' not installed" | StrudelDirt's default-synths use Mouse UGens, not built into this scsynth | harmless on a headless Pi (no mouse to read); that one synth just won't load |
| Rust build fails on `glib-sys`/`gobject-sys` | Tauri's GTK deps not installed | §1 apt packages (or build `--no-default-features` on `headless-setup`) |
