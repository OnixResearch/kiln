(import "../iroh-workflow.ncl") (
  (import "./iroh-workflow-inputs.ncl")
  & {
    allowed_user_ids | force = [1000, 1000],
  }
)
