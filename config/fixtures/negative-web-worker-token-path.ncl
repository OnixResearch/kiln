let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { token_file | force = "/srv/kiln/repository/token" })
