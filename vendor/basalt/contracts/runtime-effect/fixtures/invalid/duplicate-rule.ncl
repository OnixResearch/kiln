let H = import "../helpers.ncl" in
H.policy (H.rules @ [{ rule_id = "filesystem-read-input", scope = "filesystem", operation = "read", match_mode = "exact", subject_pattern = "cap://workspace/input", priority = H.rule_priority, decision = "deny" }]) "deny" H.identity.generation H.identity.agentos_revision H.non_claims
