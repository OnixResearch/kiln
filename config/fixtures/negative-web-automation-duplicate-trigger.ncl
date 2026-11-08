let positive = import "web-automation-positive.ncl" in
(import "../web-automation.ncl") (positive & { triggers | force = [positive.triggers [0], positive.triggers [0]] })
