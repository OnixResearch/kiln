let positive = import "hosted-pipeline-triggers-positive.ncl" in
(import "../hosted-pipeline.ncl") (positive & { schedules | force = positive.schedules & { nightly | force = positive.schedules.nightly & { cron | force = "H 2 * * *\u{0}" } } })
