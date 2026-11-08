let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { execution | force = input.execution & { probe_program | force = "/opt/true" } })
