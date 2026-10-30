# A worker may declare that it runs untrusted bindings in a per-job microVM.
let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { untrusted_capable = true })
