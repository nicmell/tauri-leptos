// The pipe's web worker: the app's wasm, run on the port that the page sends.
const port = new Promise((resolve) => {
  self.onmessage = ({ ports }) => resolve(ports[0])
})
const { default: init, worker_main } = await import('__GLUE__')
await init({ module_or_path: '__WASM__' })
worker_main(await port)
