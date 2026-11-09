let positive = import "hosted-pipeline-triggers-positive.ncl" in
let trigger = { event = "push", branches = ["main", "main"] } in
(import "../hosted-pipeline.ncl") (positive & { triggers | force = ["manual", trigger] })
