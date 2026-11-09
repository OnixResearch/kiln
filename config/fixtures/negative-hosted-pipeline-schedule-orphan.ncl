let positive = import "hosted-pipeline-triggers-positive.ncl" in
(import "../hosted-pipeline.ncl") (positive & { schedules | force = positive.schedules & { orphan | force = positive.schedules.nightly } })
