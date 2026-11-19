# Kiln must post with its own From, not a listed machine's address.
(import "./fleet-positive.ncl")
& { swarm.from | force = "britton <britton-desktop@swarm.local>" }
