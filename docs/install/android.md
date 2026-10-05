# Android

## Prerequisites

- Android Studio or the command-line SDK, with the SDK, the NDK and JDK 17 or later
- `ANDROID_HOME` and `NDK_HOME`, set to the SDK and the NDK
- The Rust targets:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi \
  i686-linux-android x86_64-linux-android
```

## How the app runs on Android

The app works as it does on desktop. The window loads `http://leptos.localhost/`, and the plugin answers inside the app. Release builds dispatch to the router in process, with the site that Tauri embeds in the APK. Dev builds forward to the watch on your computer.

## Project generation

`cargo tauri android init` generated `src-tauri/gen/android`, and the repository commits it. It is the Android project: the Gradle files, the activity and the signing setup live there. The rename script also moves its package.

One file there is a hand edit: `res/xml/network_security_config.xml`, with its reference in `AndroidManifest.xml`. Release builds refuse cleartext to every host, loopback included. The file allows it to two hosts only:

- `127.0.0.1`, for the worker's websocket to the app's own server
- `leptos.localhost`, for the dev live-reload socket. With the file present, debug builds follow it too, and they lose the general cleartext permission of `usesCleartextTraffic`.

If you run `cargo tauri android init` again, it drops these edits. Check the git diff afterwards.

## Develop

Build the site once, then start dev with an emulator or a device connected:

```bash
cargo leptos build
cargo tauri android dev
```

The first build comes before `cargo tauri android dev`, because tauri-cli waits only 180 s for the dev server.

- On an emulator, tauri-cli forwards the `devUrl` port with `adb reverse`. The `beforeDevCommand` also forwards port 3001, the live-reload socket.
- On a physical device, tauri-cli points `devUrl` at your computer's LAN address and exports that address as `TAURI_DEV_HOST`. The `beforeDevCommand` then starts the watch on it. The device and the computer must be on the same network.

With tauri-cli 2.11.4, `cargo tauri android dev` can stop after Gradle builds the APK, without an install. If that happens, keep the command running and install the app yourself:

```bash
adb install -r src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
adb shell am start -n com.example.tauri_leptos/.MainActivity
```

## Build

```bash
cargo tauri android build --debug --target aarch64
cargo tauri android build            # release, needs signing
```

The `beforeBuildCommand` builds the release site before the app. For release signing, follow <https://v2.tauri.app/distribute/sign/android/>.

## Logs

tauri-plugin-log writes to logcat, and the tag of each line is its Rust module, for example `tauri_plugin_leptos_ssr::dispatch`. To see the app's lines only:

```bash
adb logcat --pid="$(adb shell pidof com.example.tauri_leptos)"
```
