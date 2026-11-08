let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { work_root | force = "/srv/kiln/repository/work" })
