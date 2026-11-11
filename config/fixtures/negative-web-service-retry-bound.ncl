# The automatic retry count is bounded; four exceeds the reviewed maximum.
let input = import "web-service-inputs.ncl" in
(import "../web-service.ncl") (
  input & { projects | force = std.array.map (fun project => project & { retry_failed = 4 }) input.projects }
)
