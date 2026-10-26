let input = import "web-service-inputs.ncl" in
input
|> std.record.remove "tls"
|> std.record.remove "operator_token_file"
|> std.record.remove "worker_token_file"
|> std.record.remove "state_directory"
