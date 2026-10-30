let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Base = import "../canonical-base.ncl" in
let Receipt = std.record.remove "canonical" Base in
(Receipt | Contract.receipt) | Contract.linked_receipt
