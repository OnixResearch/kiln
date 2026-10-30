let input = import "web-code-review-inputs.ncl" in
(import "../web-code-review.ncl") (input & { intake | force = input.intake & { jujutsu = { workspace_path = "source", executable = "/run/current-system/sw/bin/jj", max_stack = 8 } } })
