# Kiln owns this projection, not the OnixOS inventory/service implementation.
# Inputs are immutable consumer trees; plugin paths never come from ambient lookup.
{
  nixpkgs,
  consumerInputs,
  buildIdentity,
}:
assert builtins.match "[0-9a-f]{64}" buildIdentity != null;
let
  consumer = builtins.fetchTree consumerInputs.consumer;
  wasmSource = builtins.fetchTree consumerInputs.wasmSource;
  plugin = builtins.fetchTree consumerInputs.plugin;
  wasm = import "${wasmSource}/nix/wasm.nix" { plugins = plugin; };
  shim = import "${consumer}/nix/lib.nix" {
    inherit wasm;
    inherit (nixpkgs) lib;
    onixLib = "${consumer}/lib";
  };
  # r[impl kiln.onixos_pilot.closure.construction]
  inventory = builtins.toFile "kiln-onixos-home-inventory.ncl" ''
    let original = import "${consumer}/examples/home-activation/inventory.ncl" in
    original & {
      instances.demo-home.roles.default.settings.git_user | force
        = "Alice Example (Kiln CI ${buildIdentity})",
    }
  '';
  negativeService = builtins.toFile "kiln-onixos-home-negative.ncl" ''
    fun onix =>
    let original = (import "${consumer}/examples/home-activation/services/home-tools.ncl") onix in
    original & {
      impl | force = fun args => (original.impl args) & {
        output.home.users."%{args.settings.user}".files.invalid-child = {
          target = ".gitconfig/invalid-child",
          text = "This child cannot be created beneath the real generated Git configuration.\n",
        },
      },
    }
  '';
  artifacts =
    negative:
    let
      outputs = shim.mkFlake {
        inputs = { inherit nixpkgs; };
        inventoryFile = inventory;
        modules.home-tools.file =
          if negative then
            negativeService
          else
            "${consumer}/examples/home-activation/services/home-tools.ncl";
      };
      home = outputs.homeEnvironments.workstation.alice;
    in
    {
      home-activation = outputs.apps.x86_64-linux.onix-home-workstation-alice.package;
      activation = home.activationPackage;
      home-files = home.filesPackage;
    };
  consumers = {
    success = artifacts false;
    build-failure = artifacts true;
  };
in
{
  # Preparation observes these identities, but must not realise any of them.
  preparation = consumers // {
    evaluation-failure = consumers.success;
  };
  cases = nixpkgs.lib.mapAttrs (_: selected: { inherit (selected) home-activation; }) consumers // {
    evaluation-failure = throw "kiln-onixos-pilot: explicit evaluation failure control";
  };
}
