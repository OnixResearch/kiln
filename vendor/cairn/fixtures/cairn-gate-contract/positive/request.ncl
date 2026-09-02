let Contract = import "../../../contracts/cairn-gate-contract.ncl" in
let Fixtures = import "../producer/fixtures-v1.json" in
Fixtures.request | Contract.gate_request
