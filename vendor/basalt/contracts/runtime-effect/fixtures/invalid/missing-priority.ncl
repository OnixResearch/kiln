let H = import "../helpers.ncl" in
H.policy [{ rule_id = "missing-priority", scope = "filesystem", operation = "read", match_mode = "exact", subject_pattern = "cap://workspace/input", decision = "allow" }] "deny" H.identity.generation H.identity.agentos_revision H.non_claims
