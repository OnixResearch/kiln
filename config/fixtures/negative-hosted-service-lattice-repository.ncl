let positive = import "hosted-service-lattice-positive.ncl" in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = std.array.map (fun policy => policy & { web_repository | force = "bad\u{0001}repository" }) positive.lattice_execution })
