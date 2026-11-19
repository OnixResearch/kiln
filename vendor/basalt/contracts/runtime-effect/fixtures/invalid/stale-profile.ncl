let H = import "../helpers.ncl" in
let StaleGeneration = H.identity.generation + 1 in
H.policy H.rules "deny" StaleGeneration H.identity.agentos_revision H.non_claims
