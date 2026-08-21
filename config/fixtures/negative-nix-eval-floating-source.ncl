let Source = import "./nix-eval-jobs-positive.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "source_path" "github:NixOS/nixpkgs" Source.request | std.contract.from_predicate (fun value => value == Source.request)
