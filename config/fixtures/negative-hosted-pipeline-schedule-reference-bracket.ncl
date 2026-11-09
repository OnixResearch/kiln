let positive = import "hosted-pipeline-triggers-positive.ncl" in
(import "../hosted-pipeline.ncl") (positive & { schedules | force = positive.schedules & { nightly | force = positive.schedules.nightly & { reference | force = "refs/heads/foo[bar" } } })
