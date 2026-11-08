let input = import "web-worker-inputs.ncl" in
let step = input.steps [0] in
(import "../web-worker.ncl") (input & { steps | force = [step & { cache | force = { producer = "p", toolchain = "t", platform = "linux", inputs = ["a", "a/b"], paths = ["b"], maximum_entry_bytes = 1, maximum_files = 1 } }] })
