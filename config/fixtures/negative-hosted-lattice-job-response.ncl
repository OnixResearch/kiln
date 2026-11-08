let positive = import "hosted-lattice-job-positive.ncl" in
(import "../hosted-lattice-job.ncl") (positive & { web.maximum_response_bytes | force = 1048577 })
