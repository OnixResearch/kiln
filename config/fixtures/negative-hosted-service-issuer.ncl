let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { secrets | force = positive.secrets & { issuer | force = "https://issuer.example\n" } })
