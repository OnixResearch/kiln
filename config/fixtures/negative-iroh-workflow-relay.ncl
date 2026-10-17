(import "../iroh-workflow.ncl") (
  (import "./iroh-workflow-inputs.ncl")
  & {
    relay_urls | force = ["http://relay.invalid"],
  }
)
