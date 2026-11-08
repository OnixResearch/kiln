let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { execution | force = input.execution & { read_only_data_mount_roots | force = ["/etc"] } })
