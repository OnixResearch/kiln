let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../base.ncl") & {
  observation.sigrefs_commit_after | force = "7777777777777777777777777777777777777777",
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
