let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../base.ncl") & {
  observation.feature_level | force = "root",
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
