let positive = import "../hosted-pipeline-example.ncl" in
let template = positive.templates.check in
(import "../hosted-pipeline.ncl") (
  positive
  & {
    templates | force =
      positive.templates
      & {
        check | force =
          template
          & {
            steps | force = [
              { kind = "use", step = "greet", parameters = { unknown = "x" } },
              std.array.at 1 template.steps,
            ]
          }
      }
  }
)
