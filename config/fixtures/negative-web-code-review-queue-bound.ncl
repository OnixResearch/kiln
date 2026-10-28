let input = import "web-code-review-inputs.ncl" in
(import "../web-code-review.ncl") (input & { queue | force = input.queue & { max_candidate_builds | force = 0 } })
