# The untrusted-capable declaration is a boolean.
let input = import "web-service-inputs.ncl" in
(import "../web-service.ncl") (
  input & { projects | force = std.array.map (fun project => project & { untrusted_capable = "yes" }) input.projects }
)
