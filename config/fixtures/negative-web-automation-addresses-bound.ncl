let positive = import "web-automation-positive.ncl" in
let notification = positive.notifications [0] in
(import "../web-automation.ncl") (positive & { notifications | force = [notification & { addresses | force = ["127.0.0.1", "127.0.0.2", "127.0.0.3", "127.0.0.4", "127.0.0.5"] }] })
