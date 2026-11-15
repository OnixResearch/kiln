let Profile = (import "../nix-build-profile-contract.ncl").Profile in
let build_profile =
  (import "../nix-build-profile.ncl").profile
  & {
    derivation_path | force = "/nix/store/0123456789abcdef0123456789abcdef-demo.drv",
    expected_outputs | force = [
      { name = "out", path = "/tmp/not-a-store-output" },
    ],
  }
in
{ profile | Profile = build_profile }
