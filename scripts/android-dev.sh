#!/usr/bin/env sh
# Android dev attach: the device reaches the host's dev server through
# adb reverse (3000 watch, 3002 leptos reload socket), so
# `tauri android dev` works like desktop dev — no in-process server, no
# release site build.
set -e
cd "$(dirname "$0")/.."

adb reverse tcp:3000 tcp:3000
adb reverse tcp:3002 tcp:3002

exec cargo tauri android dev "$@"
