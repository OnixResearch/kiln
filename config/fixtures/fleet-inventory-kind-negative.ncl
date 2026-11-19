# Gemini is a herdr kind that Kiln does not admit.
(import "../fleet-inventory.ncl")
& {
  executable = "/run/current-system/sw/bin/herdr",
  environment = { home = "/home/kiln", path = "/run/current-system/sw/bin" },
  machines = [
    {
      label = "aspen1",
      ssh_target = "brittonr@aspen1",
      worktree_root = "/home/kiln/worktrees",
      agents = [{ kind = 'gemini, slots = 1 }],
    },
  ],
}
