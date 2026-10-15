let Contract = import "../../../contracts/cairn-gate-contract.ncl" in
let Fixtures = import "../producer/fixtures-v1.json" in
Fixtures.pass_result | Contract.gate_result
