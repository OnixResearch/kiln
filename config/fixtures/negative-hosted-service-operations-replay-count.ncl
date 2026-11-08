let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { operations | force = positive.operations & { replay_roots | force = std.array.generate (fun index => "/private/replay" ++ std.string.from_number index) 129 } })
