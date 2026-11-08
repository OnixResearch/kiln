let positive = import "web-service-positive.ncl" in
let project = positive.projects [0] in
let node = project.workflow.nodes [0] in
(import "../web-service.ncl") (positive & { projects | force = [project & { workflow | force = project.workflow & { nodes | force = [node & { kind | force = "custom" }] } }] })
