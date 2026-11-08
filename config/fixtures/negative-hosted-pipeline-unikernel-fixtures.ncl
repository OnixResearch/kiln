let positive = import "hosted-pipeline-unikernel-positive.ncl" in
let job = positive.jobs.check in
let file = { path = "fixture", blake3 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" } in
(import "../hosted-pipeline.ncl") (positive & { jobs | force = positive.jobs & { check | force = job & { unikernel | force = job.unikernel & { fixtures | force = [file, file] } } } })
