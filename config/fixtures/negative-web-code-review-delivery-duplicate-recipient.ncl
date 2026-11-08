let positive = import "web-code-review-delivery-positive.ncl" in
let delivery = positive.delivery in
(import "../web-code-review.ncl") (positive & { delivery | force = delivery & { recipients | force = [delivery.recipients [0], delivery.recipients [0]] } })
