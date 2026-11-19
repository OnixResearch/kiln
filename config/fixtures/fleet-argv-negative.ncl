# Placeholders are whole arguments only.
(import "./fleet-positive.ncl")
& { capture.verify_argv | force = ["/bin/verify", "--attempt={attempt}"] }
