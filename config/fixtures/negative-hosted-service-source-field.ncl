let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { source_profiles | force = [positive.source_profiles [0] & { extra = 1 }] })
