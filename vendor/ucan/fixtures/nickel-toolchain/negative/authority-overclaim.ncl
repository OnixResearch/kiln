let evidence = { evaluator = "1.17.0", claim_authority = true } in
if evidence.claim_authority then error "authority-overclaim-detected" else "bounded"
