let positive = import "hosted-lattice-job-positive.ncl" in
(import "../hosted-lattice-job.ncl") (positive & { network.bind_address | force = "localhost:4444" })
