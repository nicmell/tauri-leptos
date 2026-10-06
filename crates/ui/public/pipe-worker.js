// The pipe's web worker: the app's wasm, run on the two ports that the page
// sends. self.onmessage must be set before the first await. Otherwise the
// ports can arrive while the glue loads, and nothing hears them.
const ports = new Promise((resolve) => {
  self.onmessage = ({ ports }) => resolve(ports)
})
const { default: init, worker_main } = await import('/pkg/tauri-leptos.js')
await init({ module_or_path: '/pkg/tauri-leptos.wasm' })
const [page, socket] = await ports
worker_main(page, socket)
