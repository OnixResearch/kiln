let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = (import "../base.ncl") & {
  observation.signer_node_id | force = "z6MkOtherSigner",
} in
(Receipt | Contract.receipt) | Contract.linked_receipt
