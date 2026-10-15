let Contract = import "../../../contracts/cairn-gate-contract.ncl" in
let Fixtures = import "../producer/fixtures-v1.json" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "identity" "not-a-blake3-identity" Fixtures.request | Contract.gate_request
