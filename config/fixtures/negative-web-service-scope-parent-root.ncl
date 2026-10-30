let input = import "web-service-inputs.ncl" in
let project = std.array.first input.projects in
(import "../web-service.ncl") (input & { projects | force = [project & { scope = { root = "crates/../secret" } }] })
