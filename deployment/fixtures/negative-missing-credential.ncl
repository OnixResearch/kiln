let build = import "../systemd.ncl" in
let input = import "inputs.ncl" in
build (std.record.insert "credential_sources" (std.record.remove "operator-token" input.credential_sources) (std.record.remove "credential_sources" input))
