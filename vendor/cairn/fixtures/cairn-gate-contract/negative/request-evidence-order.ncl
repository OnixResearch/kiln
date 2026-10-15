let Contract = import "../../../contracts/cairn-gate-contract.ncl" in
let Fixtures = import "../producer/fixtures-v1.json" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let evidence = [
  "b3:9999999999999999999999999999999999999999999999999999999999999999",
  "b3:8888888888888888888888888888888888888888888888888888888888888888",
] in
let applicability = replace "evidence_identities" evidence Fixtures.request.applicability in
replace "applicability" applicability Fixtures.request | Contract.gate_request
