let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../canonical-base.ncl") & {
  canonical.reference | force = "refs/heads/other",
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
