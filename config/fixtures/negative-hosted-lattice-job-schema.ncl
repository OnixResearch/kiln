let positive = import "hosted-lattice-job-positive.ncl" in
(import "../hosted-lattice-job.ncl") (positive & { schema | force = "kiln.hosted.lattice-job.v2" })
