// The page's end of a pipe to a web worker, which holds a websocket to /ws.
export function open() {
  const { port1, port2 } = new MessageChannel()
  new Worker('/pipe/worker.js', { type: 'module' }).postMessage(null, [port2])
  return port1
}
