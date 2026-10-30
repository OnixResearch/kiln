let Contract = import "../../../contracts/radicle-source-contract.ncl" in
let Base = import "../canonical-base.ncl" in
let Receipt = Base & { request | force = std.record.remove "canonical" Base.request } in
(Receipt | Contract.receipt) | Contract.linked_receipt
