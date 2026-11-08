let positive = import "web-code-review-positive.ncl" in
let rule = positive.policy.ownership.rules [0] in
(import "../web-code-review.ncl") (positive & { policy | force = positive.policy & { ownership | force = positive.policy.ownership & { rules | force = [rule & { owners | force = ["outsider"] }] } } })
