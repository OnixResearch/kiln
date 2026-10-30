# A project may declare its execution target untrusted-capable.
let input = import "web-service-inputs.ncl" in
(import "../web-service.ncl") (
  input & { projects | force = std.array.map (fun project => project & { untrusted_capable = true }) input.projects }
)
