# At most 64 noise rules.
(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    review = {
      noise = std.array.generate (fun index => { id = "rule-%{std.to_string index}", pattern = "vendor/**", reason = "Bound" }) 65,
    },
  }
)
