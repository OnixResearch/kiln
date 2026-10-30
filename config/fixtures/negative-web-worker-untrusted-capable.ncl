# The untrusted-capable declaration is a boolean.
let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { untrusted_capable = "yes" })
