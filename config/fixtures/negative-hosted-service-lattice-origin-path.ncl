let positive = import "hosted-service-lattice-positive.ncl" in
(import "../hosted-service.ncl") (positive & { lattice_execution | force = std.array.map (fun policy => policy & { web_service.service_origin | force = "https://web.example.test/api" }) positive.lattice_execution })
