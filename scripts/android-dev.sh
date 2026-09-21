#!/usr/bin/env sh
# Android dev attach: the shell's dev run (cfg(dev): api in-process,
# pages/assets reverse-proxied from the host's `cargo leptos watch`),
# reached through adb reverse — 3001 (watch), 3002 (leptos reload
# socket).
set -e
cd "$(dirname "$0")/.."

adb reverse tcp:3001 tcp:3001
adb reverse tcp:3002 tcp:3002

exec cargo tauri android dev "$@"
