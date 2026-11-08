let positive = import "web-code-review-intake-positive.ncl" in
let intake = positive.intake in
(import "../web-code-review.ncl") (positive & { intake | force = intake & { mappings | force = [intake.mappings [0], intake.mappings [0]] } })
