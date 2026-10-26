let build = import "../web-systemd.ncl" in
let input = import "web-systemd-inputs.ncl" in
build (input & { state_directory = "/tmp/accidental-new-state" })
