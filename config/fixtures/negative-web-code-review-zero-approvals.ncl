let input = import "web-code-review-inputs.ncl" in
(import "../web-code-review.ncl") (input & { policy | force = input.policy & { minimum_approvals | force = 0 } })
