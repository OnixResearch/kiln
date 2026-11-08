let input = import "web-worker-inputs.ncl" in
let step = input.steps [0] in
(import "../web-worker.ncl") (input & { steps | force = [step & { artifacts | force = ["a/result", "b/result"] }] })
