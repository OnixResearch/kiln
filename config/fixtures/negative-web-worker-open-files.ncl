let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { execution | force = input.execution & { maximum_open_files | force = 15 } })
