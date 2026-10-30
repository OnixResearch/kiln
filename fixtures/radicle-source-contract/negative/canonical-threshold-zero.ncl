let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../canonical-base.ncl") & {
  canonical.threshold | force = 0,
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
