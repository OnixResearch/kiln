let positive = import "web-automation-positive.ncl" in
(import "../web-automation.ncl") (positive & { schedules | force = std.array.generate (fun index => { id = "s" ++ std.string.from_number index, project_id = "fixture", reference = "refs/heads/main", enabled = false, interval_ms = 60000, min_interval_ms = 1000, max_interval_ms = 3600000, next_due_ms = null }) 129 })
