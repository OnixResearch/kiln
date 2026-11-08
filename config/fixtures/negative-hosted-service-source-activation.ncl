let positive = import "hosted-service-positive.ncl" in
let source = positive.source_profiles [0] & { network_activation | force = true, price_id | force = null, class | force = null, max_units | force = null } in
(import "../hosted-service.ncl") (positive & { source_profiles | force = [source] })
