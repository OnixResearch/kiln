# Exported to fixtures/review-queue-wait/load.json for the Rust queue-wait fixture.
let contracts = import "../review-queue-wait-contracts.ncl" in
contracts.example | contracts.Load | contracts.Consistent
