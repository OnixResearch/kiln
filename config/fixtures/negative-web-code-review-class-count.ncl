let positive = import "web-code-review-positive.ncl" in
(import "../web-code-review.ncl") (positive & { policy | force = positive.policy & { ownership | force = positive.policy.ownership & { classes | force = std.array.generate (fun index => { id = "c" ++ std.string.from_number index, paths = ["docs/**"] }) 9 } } })
