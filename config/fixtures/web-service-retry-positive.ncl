# A project may enable a bounded automatic retry of failed nodes on the exact source.
let input = import "web-service-inputs.ncl" in
(import "../web-service.ncl") (
  input & { projects | force = std.array.map (fun project => project & { retry_failed = 2 }) input.projects }
)
