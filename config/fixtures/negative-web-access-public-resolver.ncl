let input = import "web-access-inputs.ncl" in
(import "../web-access.ncl") (
  input
  & {
    proxy | force = input.proxy & { listen_address | force = "0.0.0.0:8099" },
  }
)
