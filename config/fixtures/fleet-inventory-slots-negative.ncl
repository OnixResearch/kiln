# A configured kind needs at least one slot.
(import "../fleet-inventory.ncl")
& {
  executable = "/run/current-system/sw/bin/herdr",
  environment = { home = "/home/kiln", path = "/run/current-system/sw/bin" },
  machines = [
    {
      label = "aspen1",
      ssh_target = "brittonr@aspen1",
      worktree_root = "/home/kiln/worktrees",
      agents = [{ kind = 'omp, slots = 0 }],
    },
  ],
}
