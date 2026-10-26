let build = import "../systemd.ncl" in
let input = import "inputs.ncl" in
build (std.record.insert "profile_path" "/etc/kiln/profile.json;id" (std.record.remove "profile_path" input))
