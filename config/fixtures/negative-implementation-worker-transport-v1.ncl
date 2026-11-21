# Transport v1 has no native request profile; the campaign requires v2.
(import "./implementation-worker-positive.ncl")
& { transport.schema | force = "kiln.lattice-implementation-cli.v1" }
