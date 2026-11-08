let positive = import "hosted-pipeline-triggers-positive.ncl" in
let job = positive.jobs.check in
let m = { a = ["v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13", "v14", "v15"], b = ["v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13", "v14", "v15"] } in
(import "../hosted-pipeline.ncl") (
  positive
  & {
    jobs | force =
      positive.jobs
      & {
        check | force = job & { matrix | force = m },
        other | force = job & { matrix | force = m },
      }
  }
)
