#!/bin/sh
# Bundle the cargo-leptos output as a single archive for Android: APK asset
# directories cannot be enumerated at runtime, one tar file can.
set -eu
cd "$(dirname "$0")/.."
[ -d target/site ] || { echo "target/site missing - run 'cargo leptos build --release' first" >&2; exit 1; }
tar -C target/site -cf target/site.tar .
echo "wrote target/site.tar"
