let positive = import "web-code-review-positive.ncl" in
(import "../web-code-review.ncl") (positive & { policy | force = positive.policy & { optional_ci | force = [{ project = "fixture", paths = std.array.generate (fun index => "src/" ++ std.string.from_number index ++ "/**") 33 }] } })
