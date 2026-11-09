let positive = import "hosted-pipeline-triggers-positive.ncl" in
(import "../hosted-pipeline.ncl") (positive & { jobs | force = positive.jobs & { "bad/key" | force = positive.jobs.check } })
