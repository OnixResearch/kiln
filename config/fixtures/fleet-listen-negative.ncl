# Only loopback and tailnet addresses may be served.
(import "./fleet-positive.ncl") & { listen | force = ["0.0.0.0:8095"] }
