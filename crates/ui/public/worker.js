import init, { worker_main } from '/pkg/tauri-leptos.js'
await init({ module_or_path: '/pkg/tauri-leptos.wasm' })
worker_main()
