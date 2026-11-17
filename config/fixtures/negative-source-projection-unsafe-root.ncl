let contracts = import "../source-projection-contracts.ncl" in
(contracts.example & { selection.root | force = "../outside" })
  | contracts.Definition
