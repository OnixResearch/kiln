let positive = import "hosted-pipeline-triggers-positive.ncl" in
let job = positive.jobs.check in
(import "../hosted-pipeline.ncl") (positive & { jobs | force = positive.jobs & { check | force = job & { steps | force = [{ kind = "run", run = "true", working_directory = "a\u{0}b" }] } } })
