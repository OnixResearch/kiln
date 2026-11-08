let positive = import "hosted-lattice-job-positive.ncl" in
(import "../hosted-lattice-job.ncl") (positive & { node_id | force = "bad node" })
