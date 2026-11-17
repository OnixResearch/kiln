let contracts = import "../source-projection-contracts.ncl" in
(contracts.example & { required_capabilities | force = ['executable_bit] })
  | contracts.Definition
