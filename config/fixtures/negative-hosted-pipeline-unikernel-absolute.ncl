let positive = import "hosted-pipeline-unikernel-positive.ncl" in
let job = positive.jobs.check in
(import "../hosted-pipeline.ncl") (positive & { jobs | force = positive.jobs & { check | force = job & { unikernel | force = job.unikernel & { fixtures | force = [{ path = "/abs", blake3 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }] } } } })
