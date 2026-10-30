let input = import "web-service-inputs.ncl" in
let project = std.array.first input.projects in
let max_scope_patterns = 32 in
let patterns = std.array.generate (fun index => "src/%{std.string.from_number index}/**") (max_scope_patterns + 1) in
(import "../web-service.ncl") (input & { projects | force = [project & { scope = { root = "crates/a", "include" = patterns } }] })
