let contracts = import "../source-projection-contracts.ncl" in
(
  contracts.example
  & {
    profile | force = 'navigation,
    required_capabilities | force = [],
    creation_destinations | force = ["notes"],
  }
)
  | contracts.Definition
