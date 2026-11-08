let positive = import "hosted-service-positive.ncl" in
let lattice = positive.lattice_execution [0] in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = [lattice & { source_limits | force = lattice.source_limits & { extra = 1 } }] })
