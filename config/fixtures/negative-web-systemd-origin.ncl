let build = import "../web-systemd.ncl" in
let input = import "web-systemd-inputs.ncl" in
build (std.record.insert "origin" "http://localhost:9443" (std.record.remove "origin" input))
