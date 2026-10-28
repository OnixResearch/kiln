let positive = import "hosted-client-positive.ncl" in
(import "../hosted-client.ncl") (
  positive
  & {
    listen_address | force = "0.0.0.0:44881",
  }
)
