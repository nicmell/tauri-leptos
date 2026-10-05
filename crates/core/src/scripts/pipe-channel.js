// The page's end of a pipe to the Tauri app's core, over Tauri IPC.
export function open() {
  const { invoke, Channel } = window.__TAURI__.core
  const { port1, port2 } = new MessageChannel()
  const events = new Channel()
  events.onmessage = (event) => port2.postMessage(event)
  const id = invoke('pipe_open', { events })
  // One call at a time: concurrent invokes can reach the session out of order.
  let queue = id
  port2.onmessage = ({ data }) => {
    queue = queue
      .then((id) => invoke('pipe_post', { id, data }).then(() => id))
      .catch((error) => {
        console.error(error)
        return id
      })
  }
  return port1
}
