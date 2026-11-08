let positive = import "web-code-review-positive.ncl" in
(import "../web-code-review.ncl") (positive & { policy | force = positive.policy & { ownership | force = positive.policy.ownership & { teams | force = { maintainers = ["outsider"] } } } })
