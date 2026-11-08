let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { secrets | force = positive.secrets & { resolvers | force = [positive.secrets.resolvers [0], positive.secrets.resolvers [0]] } })
