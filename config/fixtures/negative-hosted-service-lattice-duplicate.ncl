let positive = import "hosted-service-lattice-positive.ncl" in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = [positive.lattice_execution [0], positive.lattice_execution [0]] })
