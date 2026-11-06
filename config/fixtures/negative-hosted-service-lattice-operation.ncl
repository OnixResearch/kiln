let positive = import "hosted-service-lattice-positive.ncl" in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = std.array.map (fun policy => policy & { operation_identity | force = "not-a-content-id" }) positive.lattice_execution })
