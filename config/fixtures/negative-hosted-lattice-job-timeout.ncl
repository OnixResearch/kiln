let positive = import "hosted-lattice-job-positive.ncl" in
(import "../hosted-lattice-job.ncl") (positive & { maximum_run_ms | force = 0 })
