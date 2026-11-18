# Exported to fixtures/workspace-corpus/corpus.json for the local baseline.
let contracts = import "../workspace-corpus-contracts.ncl" in
contracts.example | contracts.Corpus | contracts.Consistent
