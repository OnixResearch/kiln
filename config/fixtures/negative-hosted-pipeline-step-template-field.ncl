let positive = import "hosted-pipeline-triggers-positive.ncl" in
let step = positive.steps.greet in
(import "../hosted-pipeline.ncl") (positive & { steps | force = positive.steps & { greet | force = step & { extra = 1 } } })
