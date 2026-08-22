let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../base.ncl") & {
  snapshot.revision | force = "8888888888888888888888888888888888888888",
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
