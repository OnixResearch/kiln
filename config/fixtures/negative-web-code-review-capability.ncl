(import "../web-code-review.ncl") ((import "web-code-review-inputs.ncl") & { provider | force = (import "web-code-review-inputs.ncl").provider & { capabilities | force = ["inspect"] } })
