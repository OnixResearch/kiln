(import "../web-access.ncl") ((import "web-access-inputs.ncl") & { proxy | force = (import "web-access-inputs.ncl").proxy & { listen_address | force = "127.0.0.1:0" } })
