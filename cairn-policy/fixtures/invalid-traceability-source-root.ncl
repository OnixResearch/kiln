let Contracts = import "../contracts.ncl" in
let Base = import "../generated/cairn-policy.json" in
(Base & {
  traceability_policy | force = {
    default_profile = "cairn-default",
    profiles = [{
      id = "cairn-default",
      requirement_sources = [{
        root = ".cairn/specs",
        source_kind = "accepted_specs",
        extensions = ["md"],
      }],
      evidence_sources = [{
        root = "../src",
        source_kind = "rust_source",
        extensions = ["rs"],
      }],
      marker_verbs = ["impl", "verify", "depends", "related"],
    }],
  },
}) | Contracts.Policy
