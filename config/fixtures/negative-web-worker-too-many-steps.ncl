let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { steps | force = std.array.generate (fun index => { node_id = "n" ++ std.string.from_number index, program = "/usr/bin/true", argv = [], environment = {}, artifacts = [] }) 129 })
