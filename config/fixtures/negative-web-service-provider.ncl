let positive = import "web-service-inputs.ncl" in
(import "../web-service.ncl") (positive & { projects | force = std.array.map (fun project => project & { execution_provider = "hosted" }) positive.projects })
