let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
let runner = execution.runners.microvm in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { runners | force = execution.runners & { microvm | force = runner & { debug_hold | force = runner.debug_hold & { max_concurrent | force = 2 } } } } })
