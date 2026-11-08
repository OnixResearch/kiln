let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { operations | force = positive.operations & { extra = 1 } })
