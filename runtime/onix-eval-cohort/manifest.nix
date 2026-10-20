# Exact provenance for the separately selected Onix discovery profile.
# Runtime source is imported verbatim by scripts/import-onix-discovery-fixture.sh.
{ evaluator, plugin, jobset }:
let
  component = path: identity: {
    library = {
      contract_version = 1;
      provider_namespace = "nix";
      native_kind = "shared-library";
      native_id = path;
    };
    inherit identity;
  };
in {
  schema = "kiln.nix-eval-jobs-cohort.v1";
  upstream_revision = "9f1553afabd95f9f59951f5c946e86005af7196a";
  adapter_version = "2.36.4";
  source_version = "0.1.6";
  nix_revision = "f93f28ad9dce007baae13aa461c16abf656d1171";
  nix_version = "2.36.0";
  nixpkgs_revision = "34ca302a9572963c02e385c056be37c85ff51b77";
  platform = "x86_64-linux";
  executable = "${evaluator}/bin/nix-eval-jobs";
  onix = {
    evaluator_kind = "onix-nix";
    evaluator_source = "https://github.com/NixOS/nix-eval-jobs";
    evaluator_license = "GPL-3.0";
    evaluator_nar_hash = "sha256-t1FbcjvTWQ1WO3hJNKN+viNXuz4BX9Pte5K4F+IJLDk=";
    fork_source = "https://github.com/onixcomputer/nix";
    fork_license = "LGPL-2.1-or-later";
    fork_nar_hash = "sha256-sxgxa7OyG4WPBehA05IfYrgs1REwnIS3m+SW6utVX+A=";
    api_patch_blake3 = "3dda2b2961149c553dcacc9de4a765bf0346c2baae39874f2850dc75dd03b15c";
    derivations_patch_blake3 = "9600554082a4b7e1ec8dc53f26b30ac4919bde1cdd5af7d825c1cadf3c8793a0";
    executable_identity = "4b4e221bfe3645da5f9d89198f211e07f63e4ea5f4fbd6064d6311bfa5090361";
    host_abi = "nix_wasm_init_v1";
    features = [ "flakes" "nix-command" "wasm-builtin" ];
    plugin = {
      contract_version = 1;
      provider_namespace = "nix";
      native_kind = "store-path";
      native_id = "${plugin}/nickel_plugin.wasm";
    };
    plugin_identity = "6bbf0180617b4d87746416eb8d58d09dea1076312935038b4d92706e5c7626f6";
    plugin_source = "https://github.com/onixcomputer/onix-wasm";
    plugin_revision = "54bf09574c42a21f9f38dc7e01bc44440e6fbc07";
    plugin_license = "NOASSERTION (plugin wrapper); vendored Nickel MIT";
    package_source = "https://github.com/OnixResearch/onixpkgs";
    package_revision = "3bdf51c00947ffd6b8bd3355ff83509a03752042";
    package_license = "NOASSERTION (package wrapper); evaluator GPL-3.0";
    fixture_upstream = "https://github.com/onixcomputer/onix-modules";
    fixture_license = "AGPL-3.0-or-later";
    fixture_source = {
      contract_version = 1;
      provider_namespace = "kiln-source";
      native_kind = "nix-store-path";
      native_id = "${jobset}";
    };
    fixture_source_identity = "dbb21080c65c18ce318cf95106f8b112edb1c29219700f846d0721c77d5abbfb";
    fixture_revision = "69a131d71a509f9c57646b71113f75f39ab637b1";
    components = [
      (component
        "/nix/store/xs9x9imziz5fz9d089vs64xxahz5v7iy-nix-store-2.36.0/lib/libnixstore.so.2.36.0"
        "83efc75ea9a8bddf172cdd4f2827d2b85ee986bc872e407c6f509e5cc24bff9d")
      (component
        "/nix/store/sifz76vs2qn3ng7d8bc9ll96zb58x4zv-nix-util-2.36.0/lib/libnixutil.so.2.36.0"
        "58d1aa3a8a8a28fc264ab3604d6d327f6b5b9756fec516a5b218120566fc3cd4")
      (component
        "/nix/store/43v5jy3q8lql3snvdzqwj0drvsrmihxg-nix-fetchers-2.36.0/lib/libnixfetchers.so.2.36.0"
        "8afea53fa2c1fdfe215fe86fc3c35f473a4a2f90304d2d7a7a4c7c2603629f90")
      (component
        "/nix/store/77sdbia58r9r11xy9xn9q45sgy5gvcvl-nix-expr-2.36.0/lib/libnixexpr.so.2.36.0"
        "15ab72e4ab76edcbcaa836d47a4fae5da119ead8dca70f8d1e6d2ef3ee12a1e6")
      (component
        "/nix/store/wskhfcg99q278r1fps88rk22jncndmrf-nix-flake-2.36.0/lib/libnixflake.so.2.36.0"
        "a42f60463440478c599502decf74e82e2f7310316335c01c156af65a74e50c8b")
      (component
        "/nix/store/5g13fg62n6fqzyzgvv2i0g1f8a1f4q28-nix-main-2.36.0/lib/libnixmain.so.2.36.0"
        "8cbb440566851976a87fd0c0798eec619c1d43829c8313b2fd1c22d34824b309")
      (component
        "/nix/store/l3gh3m8hzzpfk5yx73nxfpq50nr354jp-nix-cmd-2.36.0/lib/libnixcmd.so.2.36.0"
        "871233de0fe2784ad54fb4804be3a936907aee77f580c634974b47da73a0fd8f")
    ];
  };
}
