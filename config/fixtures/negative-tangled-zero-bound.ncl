# Must fail: every Tangled limit is positive.
(import "tangled-spindle-positive.ncl")
& {
  limits.maximum_pipelines | force = 0,
}
