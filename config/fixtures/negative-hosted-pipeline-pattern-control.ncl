let positive = import "hosted-pipeline-triggers-positive.ncl" in
let trigger = { event = "push", branches = ["main\n"] } in
(import "../hosted-pipeline.ncl") (positive & { triggers | force = ["manual", trigger] })
