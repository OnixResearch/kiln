let positive = import "web-service-positive.ncl" in
let project = positive.projects [0] in
(import "../web-service.ncl") (positive & { projects | force = [project & { workflow | force = project.workflow & { transport_limits | force = project.workflow.transport_limits & { maximum_frame_bytes | force = 33554433 } } }] })
