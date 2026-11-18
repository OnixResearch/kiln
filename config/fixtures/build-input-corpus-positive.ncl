# Exported to fixtures/build-inputs/corpus.json for the build-input harness.
let contracts = import "../build-input-contracts.ncl" in
contracts.example | contracts.Corpus | contracts.Consistent
