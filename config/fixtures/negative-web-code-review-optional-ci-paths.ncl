let positive = import "web-code-review-positive.ncl" in
(import "../web-code-review.ncl") (positive & { policy | force = positive.policy & { optional_ci | force = [{ project = "fixture", paths = [] }] } })
