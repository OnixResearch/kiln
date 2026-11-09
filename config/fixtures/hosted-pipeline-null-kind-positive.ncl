let positive = import "hosted-pipeline-triggers-positive.ncl" in
let job = positive.jobs.check in
(import "../hosted-pipeline.ncl") (positive & { jobs | force = positive.jobs & { check | force = job & { kind | force = null, unikernel | force = null } } })
