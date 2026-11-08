let positive = import "hosted-service-positive.ncl" in
let long = std.array.fold_left (fun acc _ => acc ++ "a") "" (std.array.generate (fun _ => 0) 129) in
(import "../hosted-service.ncl") (positive & { operations | force = positive.operations & { service_id | force = long } })
