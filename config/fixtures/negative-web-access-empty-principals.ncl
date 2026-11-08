(import "../web-access.ncl") ((import "web-access-inputs.ncl") & { proxy | force = (import "web-access-inputs.ncl").proxy & { principals | force = [] } })
