# Repositories are named owner/name.
(import "../fleet-inventory.ncl")
& {
  executable = "/run/current-system/sw/bin/herdr",
  environment = { home = "/home/kiln", path = "/run/current-system/sw/bin" },
  machines = [
    {
      label = "aspen1",
      ssh_target = "brittonr@aspen1",
      worktree_root = "/home/kiln/worktrees",
      repositories = { kiln = "/home/kiln/src/kiln" },
      agents = [{ kind = 'omp, slots = 1 }],
    },
  ],
}
