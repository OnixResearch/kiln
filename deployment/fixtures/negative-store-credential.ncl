let build = import "../systemd.ncl" in
let input = import "inputs.ncl" in
let sources = std.record.insert "tls-key" "/nix/store/not-a-private-key/key.pem" (std.record.remove "tls-key" input.credential_sources) in
build (std.record.insert "credential_sources" sources (std.record.remove "credential_sources" input))
