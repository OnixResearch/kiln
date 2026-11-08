let positive = import "web-automation-positive.ncl" in
let notification = positive.notifications [0] in
(import "../web-automation.ncl") (positive & { notifications | force = [notification & { path | force = "/completion?x=1" }] })
