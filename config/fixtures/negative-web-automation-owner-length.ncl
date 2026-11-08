let positive = import "web-automation-positive.ncl" in
let trigger = positive.triggers [0] in
let long = std.array.fold_left (fun acc _ => acc ++ "a") "" (std.array.generate (fun _ => 0) 40) in
(import "../web-automation.ncl") (positive & { triggers | force = [trigger & { repository | force = trigger.repository & { owner | force = long } }] })
