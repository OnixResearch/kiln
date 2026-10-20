# Explicit operator inputs for the isolated pilot. No credential material belongs here.
let NonEmpty = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0) in
let StorePath = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^/nix/store/[^\\x00]+$" value) in
let Tree = {
  type | std.contract.from_predicate (fun value => value == "path"),
  path | StorePath,
  narHash | NonEmpty,
} in
let Executable = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^/[^\\x00]+$" value) in
fun inputs => inputs | {
  consumer | Tree,
  consumer_revision | NonEmpty,
  wasmSource | Tree,
  plugin | Tree,
  producer_substitution_reason | NonEmpty,
  nixpkgs_url | NonEmpty,
  source_repository | NonEmpty,
  source_revision | NonEmpty,
  source_actor | NonEmpty,
  executables | {
    nix | Executable,
    nix_store | Executable,
    b3sum | Executable,
    bubblewrap | Executable,
    nickel | Executable,
    host | Executable,
    extension | Executable,
    lattice | StorePath,
    lattice_source | StorePath,
    worker | Executable,
    coordinator | Executable,
    openssl | Executable,
    shell | Executable,
    sleep | Executable,
  },
}
