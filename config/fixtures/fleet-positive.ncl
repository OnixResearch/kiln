(import "../fleet.ncl")
& {
  machine = "britton-desktop",
  listen = ["127.0.0.1:8095", "100.110.43.11:8095"],
  peers = { "aspen3-1.bison-tailor.ts.net." = "aspen3" },
  operators = ["britton-desktop"],
  authority_operator = "britton-desktop",
  tailscale = "/run/current-system/sw/bin/tailscale",
  implementer = {
    database = "/var/lib/kiln-fleet/implementer.sqlite",
    enrollment = "/etc/kiln-fleet/enrollment.json",
  },
  inventory = "/etc/kiln-fleet/fleet-inventory.json",
  swarm = {
    executable = "/run/current-system/sw/bin/iroh-news",
    state_dir = "/var/lib/kiln-fleet/swarm",
    server = "swarm.blr.dev",
    from = "kiln <kiln-britton-desktop@swarm.local>",
  },
  capture = {
    git = "/run/current-system/sw/bin/git",
    tar = "/run/current-system/sw/bin/tar",
    export_root = "/var/lib/kiln-fleet/exports",
    verification_profiles = "/etc/kiln-fleet/verification-profiles",
    repositories = {
      "OnixResearch/kiln" = { authority = "/etc/kiln-fleet/authority/kiln.json" },
    },
    verify_argv = [
      "/run/current-system/sw/bin/kiln-web-service",
      "verify-implementation",
      "--database",
      "{database}",
      "--handoff",
      "{handoff}",
      "--profile",
      "{verification_profile}",
      "--authority",
      "{authority}",
      "--local-operator",
      "britton-desktop",
    ],
  },
  dispatch = { enabled = true, kinds = ['omp, 'claude] },
}
