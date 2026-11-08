let positive = import "hosted-service-lattice-positive.ncl" in
let policy = std.array.first positive.lattice_execution in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = [policy & { web_service | force = policy.web_service & { tls_ca_path | force = "/srv/./web-ca.pem" } }] })
