// The pipe's web worker: the app's wasm, run on the port that the page sends.
// self.onmessage must be set before the first await. Otherwise the port can
// arrive while the glue loads, and nothing hears it.
const port = new Promise((resolve) => {
  self.onmessage = ({ ports }) => resolve(ports[0])
})
const { default: init, worker_main } = await import('/pkg/tauri-leptos.js')
await init({ module_or_path: '/pkg/tauri-leptos.wasm' })
worker_main(await port)
