let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Receipt = import "../base.ncl" in
(Receipt | Contract.receipt) | Contract.linked_receipt
