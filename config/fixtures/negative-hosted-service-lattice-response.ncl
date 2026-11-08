let positive = import "hosted-service-lattice-positive.ncl" in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = std.array.map (fun policy => policy & { web_service.maximum_response_bytes | force = 1048577 }) positive.lattice_execution })
