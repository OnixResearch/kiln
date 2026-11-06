let positive = import "hosted-service-lattice-positive.ncl" in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = std.array.map (fun policy => policy & { web_service.service_origin | force = "http://web.example.test" }) positive.lattice_execution })
