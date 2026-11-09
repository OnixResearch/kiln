let positive = import "hosted-pipeline-triggers-positive.ncl" in
let trigger = { event = "pull_request", branches = ["main"], types = ["opened", "opened"] } in
(import "../hosted-pipeline.ncl") (positive & { triggers | force = ["manual", trigger] })
