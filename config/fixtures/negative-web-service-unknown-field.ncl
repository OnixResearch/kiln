let input = import "web-service-inputs.ncl" in
(import "../web-service.ncl") (input & { browser_supplied_program = "/bin/sh" })
