let input = import "web-workers-inputs.ncl" in
(import "../web-workers.ncl") (input & { workers | force = [{ worker_id = "local", principal = "worker", project_id = "fixture", target_id = "not-an-identity", maximum_slots = 1 }] })
