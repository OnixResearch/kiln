# SPDX-License-Identifier: AGPL-3.0-or-later
# Discovery-only fixture: no derivation build or host-shell availability claim.
# OnixOS base: onixcomputer/onix-modules@7b9a886736d89c776d6a13986bde0d941fd29168
# Plugin: onixcomputer/onix-wasm@54bf09574c42a21f9f38dc7e01bc44440e6fbc07
# Nickel upstream: nickel-lang/nickel@1320a983e6c3d1e2fb53dd2464b084b4903b1426
# Nickel WASM patch: brittonr/nickel-wasm@5de51b8976cf6a35ae8d026a4f28916df5f259fa
# Evaluator: nix-eval-jobs@9f1553afabd95f9f59951f5c946e86005af7196a
# Fork: onixcomputer/nix@f93f28ad9dce007baae13aa461c16abf656d1171
# Package review: OnixResearch/onixpkgs@3bdf51c00947ffd6b8bd3355ff83509a03752042
{
  description = "Source-bound Onix Nickel WASM discovery fixture for Kiln";

  outputs = { self }:
    let
      plugin = "${builtins.fetchTree {
        type = "path";
        path = "/nix/store/di3jrnpmrc4lff4581vxmprmcsgswd0r-nickel-plugin-0.1.0";
        narHash = "sha256-T44jy5Ax7YawXUBgd76O5gjO1ERS6LBaU0D7axpE8+o=";
      }}/nickel_plugin.wasm";
      config =
        if !(builtins ? wasm) then
          throw "kiln-wasm-discovery: requires the pinned Onix builtins.wasm evaluator"
        else if !(builtins.pathExists plugin) then
          throw "kiln-wasm-discovery: provision the exact pinned Nickel plugin before discovery"
        else
          builtins.wasm
            {
              path = plugin;
              function = "evalNickel";
            } ''
            {
              system | String = "x86_64-linux",
              seed_name | String = "onix-nickel-seed",
              seed_message | String = "nickel-wasm-seed-v1",
              report_name | String = "onix-nickel-report",
              report_prefix | String = "onix-report",
            }
          '';
      seed = builtins.derivation {
        name = config.seed_name;
        system = config.system;
        builder = "/bin/sh";
        args = [ "-c" ''printf '%s\n' "$message" > "$out"'' ];
        message = config.seed_message;
      };
      report = builtins.derivation {
        name = config.report_name;
        system = config.system;
        builder = "/bin/sh";
        args = [ "-c" ''IFS= read -r message < "$seed"; printf '%s: %s\n' "$prefix" "$message" > "$out"'' ];
        seed = "${seed}";
        prefix = config.report_prefix;
      };
    in
    {
      jobs.x86_64-linux = {
        inherit seed report;
        broken = throw "kiln-wasm-discovery: intentional independent job error";
      };
    };
}
