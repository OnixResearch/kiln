# Machine labels are lowercase [a-z0-9][a-z0-9-]*.
(import "../fleet-inventory.ncl")
& {
  executable = "/run/current-system/sw/bin/herdr",
  environment = { home = "/home/kiln", path = "/run/current-system/sw/bin" },
  machines = [
    {
      label = "Aspen1",
      ssh_target = "brittonr@aspen1",
      worktree_root = "/home/kiln/worktrees",
      agents = [{ kind = 'omp, slots = 1 }],
    },
  ],
}
