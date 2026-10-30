let input = import "web-code-review-inputs.ncl" in
(import "../web-code-review.ncl") (input & { intake | force = input.intake & { jujutsu = { change_id_header = true, max_stack = 8, revsets = true } } })
