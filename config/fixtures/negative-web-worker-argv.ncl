let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { steps | force = [input.steps [0] & { argv | force = std.array.generate (fun index => std.string.from_number index) 4097 }] })
