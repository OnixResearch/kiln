let positive = import "hosted-pipeline-triggers-positive.ncl" in
(import "../hosted-pipeline.ncl") (positive & { name | force = "bad\u{0}name" })
