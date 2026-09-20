#!/bin/sh
# Build the installable deb (binary + site + systemd unit). Run on the
# target machine (e.g. the Pi): the frontend must be built first because
# cargo-deb only builds the Rust binary.
set -eu
cd "$(dirname "$0")/.."
cargo leptos build --release
cargo deb -p tauri-leptos-cli
echo "install with: sudo apt install ./target/debian/tauri-leptos-cli_*.deb"
