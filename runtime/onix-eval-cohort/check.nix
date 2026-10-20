{ pkgs, evaluator, plugin, manifest, jobset }:
pkgs.runCommand "kiln-onix-evaluator-cohort" {
  nativeBuildInputs = [ pkgs.b3sum pkgs.jq ];
} ''
  set -euo pipefail
  executable=${evaluator}/bin/nix-eval-jobs
  manifest=${manifest}
  plugin=${plugin}/nickel_plugin.wasm
  source=${jobset}/flake.nix
  test "$executable" = /nix/store/69k3iijnj5s8417anq7ncgc9f006gc1n-nix-eval-jobs-2.36.4/bin/nix-eval-jobs
  test "$plugin" = /nix/store/di3jrnpmrc4lff4581vxmprmcsgswd0r-nickel-plugin-0.1.0/nickel_plugin.wasm
  test "$(b3sum --no-names "$executable")" = 4b4e221bfe3645da5f9d89198f211e07f63e4ea5f4fbd6064d6311bfa5090361
  test "$(b3sum --no-names "$plugin")" = 6bbf0180617b4d87746416eb8d58d09dea1076312935038b4d92706e5c7626f6
  test "$(b3sum --no-names "$source")" = dbb21080c65c18ce318cf95106f8b112edb1c29219700f846d0721c77d5abbfb
  test "$(b3sum --no-names "$manifest")" = 1e7df147057fde6b439541427a51c2d4ba70f6a42ee15d30021d120e9dda8263
  while IFS=$'\t' read -r library identity; do
    test ! -L "$library"
    test "$(b3sum --no-names "$library")" = "$identity"
  done < <(jq -r '.onix.components[] | [.library.native_id,.identity] | @tsv' "$manifest")
  jq -e '
    .nix_revision == "f93f28ad9dce007baae13aa461c16abf656d1171" and
    .onix.fixture_revision == "69a131d71a509f9c57646b71113f75f39ab637b1" and
    .onix.features == ["flakes","nix-command","wasm-builtin"] and
    .onix.host_abi == "nix_wasm_init_v1" and
    (.onix.components | length) == 7 and
    .onix.plugin.contract_version == 1 and
    .onix.fixture_source.contract_version == 1 and
    all(.onix.components[].library; .contract_version == 1)
  ' "$manifest"
  mkdir "$out"
  cp "$manifest" "$out/cohort.json"
  b3sum "$executable" "$plugin" "$source" > "$out/measured-files.blake3"
''
