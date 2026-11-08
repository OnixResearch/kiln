let positive = import "web-automation-positive.ncl" in
let trigger = positive.triggers [0] in
(import "../web-automation.ncl") (positive & { triggers | force = [trigger & { repository | force = trigger.repository & { owner | force = "bad owner" } }] })
