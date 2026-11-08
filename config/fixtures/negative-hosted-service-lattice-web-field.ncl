let positive = import "hosted-service-positive.ncl" in
let lattice = positive.lattice_execution [0] in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = [lattice & { web_service | force = lattice.web_service & { extra = 1 } }] })
