let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (
  positive
  & {
    secrets | force = positive.secrets & { token_lifetime_seconds | force = 301 },
  }
)
