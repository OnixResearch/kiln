let input = import "web-code-review-inputs.ncl" in
(import "../web-code-review.ncl") (input & { provider | force = input.provider & { target_refs | force = ["refs/heads/../main"] } })
