let positive = import "web-automation-positive.ncl" in
(import "../web-automation.ncl") (positive & { notifications | force = [positive.notifications [0], positive.notifications [0]] })
