(import "../fleet-inventory.ncl")
& {
  executable = "/run/current-system/sw/bin/herdr",
  environment = {
    home = "/home/kiln",
    path = "/run/current-system/sw/bin",
    xdg_runtime_dir = "/run/user/1000",
  },
  machines = [
    {
      label = "aspen1",
      ssh_target = "brittonr@aspen1",
      worktree_root = "/home/kiln/worktrees",
      repositories = { "OnixResearch/kiln" = "/home/kiln/src/kiln" },
      agents = [
        { kind = 'omp, slots = 2 },
        { kind = 'claude, slots = 1 },
      ],
    },
    {
      label = "aspen3",
      herdr_machine = "aspen3",
      ssh_target = "brittonr@aspen3",
      worktree_root = "/home/brittonr/kiln-worktrees",
      repositories = { "OnixResearch/kiln" = "/home/brittonr/git/OnixResearch/kiln" },
      agents = [
        { kind = 'codex, slots = 1 },
        { kind = 'agy, slots = 1, args = [] },
      ],
    },
  ],
}
