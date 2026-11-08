let positive = import "web-code-review-positive.ncl" in
(import "../web-code-review.ncl") (positive & { policy | force = positive.policy & { ownership | force = positive.policy.ownership & { dependents | force = { fixture = std.array.generate (fun index => "project" ++ std.string.from_number index) 65 } } } })
