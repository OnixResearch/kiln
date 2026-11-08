let positive = import "web-code-review-positive.ncl" in
let rule = positive.policy.ownership.rules [0] in
(import "../web-code-review.ncl") (positive & { policy | force = positive.policy & { ownership | force = positive.policy.ownership & { rules | force = [rule & { required_evidence | force = std.array.generate (fun index => { kind = "project_ci", project = "p" ++ std.string.from_number index }) 9 }] } } })
