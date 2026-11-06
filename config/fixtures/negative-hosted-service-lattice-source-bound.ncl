let positive = import "hosted-service-lattice-positive.ncl" in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = std.array.map (fun policy => policy & { source_limits.max_total_bytes | force = 0 }) positive.lattice_execution })
