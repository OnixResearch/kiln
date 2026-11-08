let positive = import "web-code-review-delivery-positive.ncl" in
let delivery = positive.delivery in
(import "../web-code-review.ncl") (positive & { delivery | force = delivery & { destinations | force = [delivery.destinations [0], delivery.destinations [0]] } })
