let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { execution | force = input.execution & { read_only_data_mounts | force = ["/srv/a", "/srv/b", "/srv/c", "/srv/d", "/srv/e"] } })
