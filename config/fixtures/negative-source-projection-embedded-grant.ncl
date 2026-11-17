let contracts = import "../source-projection-contracts.ncl" in
(contracts.example & { grants = ["source:read"] })
  | contracts.Definition
