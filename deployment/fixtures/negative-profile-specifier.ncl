let build = import "../systemd.ncl" in
let input = import "inputs.ncl" in
build (std.record.insert "profile_path" "/etc/kiln/%n.json" (std.record.remove "profile_path" input))
