let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../canonical-base.ncl") & {
  canonical.observations | force = [{ delegate = "did:key:z6MkOther", object = "4444444444444444444444444444444444444444" }],
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
