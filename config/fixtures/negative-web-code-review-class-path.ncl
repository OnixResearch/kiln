let positive = import "web-code-review-positive.ncl" in
let class = positive.policy.ownership.classes [0] in
(import "../web-code-review.ncl") (positive & { policy | force = positive.policy & { ownership | force = positive.policy.ownership & { classes | force = [class & { paths | force = ["../escape"] }] } } })
