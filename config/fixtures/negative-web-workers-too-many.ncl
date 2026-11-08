let input = import "web-workers-inputs.ncl" in
(import "../web-workers.ncl") (input & { workers | force = std.array.generate (fun index => { worker_id = "w" ++ std.string.from_number index, principal = "worker", project_id = "fixture", target_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", maximum_slots = 1 }) 257 })
