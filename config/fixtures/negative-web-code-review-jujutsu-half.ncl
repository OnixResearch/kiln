let input = import "web-code-review-inputs.ncl" in
(import "../web-code-review.ncl") (input & { intake | force = input.intake & { jujutsu = { workspace_path = "/var/lib/kiln/source", max_stack = 8 } } })
