let positive = import "hosted-pipeline-triggers-positive.ncl" in
(import "../hosted-pipeline.ncl") (
  positive
  & {
    jobs | force = {
      a = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true" }], needs = ["b"] },
      b = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true" }], needs = ["a"] },
    }
  }
)
