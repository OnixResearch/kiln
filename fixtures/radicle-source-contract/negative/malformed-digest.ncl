let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../base.ncl") & {
  snapshot.canonical_tree_blake3 | force = "not-a-digest",
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
