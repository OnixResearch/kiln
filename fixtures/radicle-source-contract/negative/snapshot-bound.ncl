let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../base.ncl") & {
  snapshot.entry_count | force = 1025,
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
