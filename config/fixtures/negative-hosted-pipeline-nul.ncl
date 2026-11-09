let positive = import "hosted-pipeline-triggers-positive.ncl" in
(import "../hosted-pipeline.ncl") (positive & { env | force = { X = "a\u{0}b" } })
