let input = import "web-service-inputs.ncl" in
let build_profile =
  (import "../nix-build-profile.ncl").profile
  & {
    derivation_path | force = "/nix/store/0123456789abcdef0123456789abcdef-demo.drv",
    expected_outputs | force = [
      { name = "out", path = "/nix/store/fedcba9876543210fedcba9876543210-demo" },
    ],
  }
in
(import "../web-service.ncl") (
  input
  & {
    projects | force =
      std.array.map
        (fun project =>
          project
          & {
            canonical_nix_build = {
              node_id = "native-build",
              source_revision = "0123456789abcdef0123456789abcdef01234567",
              profile = build_profile,
            },
          }
        )
        input.projects,
  }
)
