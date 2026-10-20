let expected = "99b84b7c0028edabcae5796bbd0d485f4b7b7b96" in
let observed = "0000000000000000000000000000000000000000" in
if expected != observed then error "identity-mismatch-detected" else "identity-ok"
